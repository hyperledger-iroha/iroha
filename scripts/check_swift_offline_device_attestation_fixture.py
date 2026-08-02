#!/usr/bin/env python3
"""Verify the checked-in Swift offline-device-attestation Rust fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Sequence


DEFAULT_FIXTURE = Path(
    "IrohaSwift/Tests/IrohaSwiftTests/Fixtures/"
    "offline_device_attestation_abi21.json"
)
GENERATOR_SUBCOMMAND = "offline-device-attestation"
GENERATED_BY = f"kotlin-fixture-gen {GENERATOR_SUBCOMMAND}"


class FixtureError(RuntimeError):
    """Raised when generator output or the checked-in fixture is noncanonical."""


def canonical_hex(value: str, field: str, *, exact_bytes: int | None = None) -> str:
    """Validate lowercase, unprefixed, even-length hexadecimal text."""
    if not value or value != value.strip() or value.lower() != value:
        raise FixtureError(f"{field} must be non-empty canonical lowercase hex")
    try:
        decoded = bytes.fromhex(value)
    except ValueError as error:
        raise FixtureError(f"{field} must be canonical lowercase hex") from error
    if decoded.hex() != value:
        raise FixtureError(f"{field} must be unprefixed even-length lowercase hex")
    if exact_bytes is not None and len(decoded) != exact_bytes:
        raise FixtureError(f"{field} must contain exactly {exact_bytes} bytes")
    return value


def render_fixture(generator_output: bytes) -> bytes:
    """Convert the generator's strict five-line output into checked-in JSON."""
    try:
        text = generator_output.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise FixtureError("generator output must be valid UTF-8") from error
    if not text.endswith("\n"):
        raise FixtureError("generator output must end with one newline")
    lines = text.splitlines()
    if len(lines) != 5:
        raise FixtureError(
            f"generator output must contain exactly five lines, found {len(lines)}"
        )

    registration_hex = canonical_hex(lines[0], "registration_hex")
    canonical_hex(lines[1], "instruction_hex")
    challenge_hash_hex = canonical_hex(
        lines[2], "challenge_hash_hex", exact_bytes=32
    )
    account_id = lines[3]
    if not account_id or account_id != account_id.strip():
        raise FixtureError("account_id must be non-empty without surrounding whitespace")
    registration_id_hex = canonical_hex(
        lines[4], "registration_id_hex", exact_bytes=32
    )

    document = {
        "fixture": "offline_device_attestation_abi21",
        "generated_by": GENERATED_BY,
        "registration_hex": registration_hex,
        "challenge_hash_hex": challenge_hash_hex,
        "account_id": account_id,
        "registration_id_hex": registration_id_hex,
    }
    return (
        json.dumps(document, ensure_ascii=False, indent=2).encode("utf-8") + b"\n"
    )


def run_generator(command: Sequence[str]) -> bytes:
    """Run one isolated fixture-generator pass and return its stdout."""
    completed = subprocess.run(
        command,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        stderr = completed.stderr.decode("utf-8", errors="replace").strip()
        raise FixtureError(
            f"fixture generator failed with status {completed.returncode}: {stderr}"
        )
    return completed.stdout


def generator_command(generator: Path | None, cargo: str) -> list[str]:
    """Resolve the direct-binary or Cargo-driven generator command."""
    if generator is not None:
        return [str(generator), GENERATOR_SUBCOMMAND]
    return [
        cargo,
        "run",
        "--quiet",
        "--locked",
        "-p",
        "kotlin-fixture-gen",
        "--features",
        "dev-tools",
        "--bin",
        "kotlin-fixture-gen",
        "--",
        GENERATOR_SUBCOMMAND,
    ]


def verify_two_pass_output(command: Sequence[str]) -> bytes:
    """Require byte-identical raw and rendered output from two generator runs."""
    first = run_generator(command)
    second = run_generator(command)
    if first != second:
        raise FixtureError("fixture generator output changed between isolated passes")
    rendered_first = render_fixture(first)
    rendered_second = render_fixture(second)
    if rendered_first != rendered_second:
        raise FixtureError("rendered fixture changed between isolated passes")
    return rendered_first


def atomic_write_fixture(path: Path, content: bytes) -> None:
    """Durably replace one fixture through a same-directory temporary file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.",
        dir=path.parent,
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as output:
            os.fchmod(output.fileno(), 0o644)
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        directory_descriptor = os.open(
            path.parent,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0),
        )
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
    except OSError as error:
        temporary.unlink(missing_ok=True)
        raise FixtureError(f"failed to atomically write {path}: {error}") from error


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fixture",
        type=Path,
        default=DEFAULT_FIXTURE,
        help=f"checked-in Swift fixture (default: {DEFAULT_FIXTURE})",
    )
    parser.add_argument(
        "--generator",
        type=Path,
        help="already-built kotlin-fixture-gen binary; avoids invoking Cargo",
    )
    parser.add_argument(
        "--cargo",
        default="cargo",
        help="Cargo executable used only when --generator is omitted",
    )
    parser.add_argument(
        "--write",
        action="store_true",
        help="replace the fixture after the two byte-identical generator passes",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        command = generator_command(args.generator, args.cargo)
        rendered = verify_two_pass_output(command)
        if args.write:
            atomic_write_fixture(args.fixture, rendered)
        else:
            try:
                checked_in = args.fixture.read_bytes()
            except FileNotFoundError as error:
                raise FixtureError(
                    f"required checked-in fixture is missing: {args.fixture}"
                ) from error
            if checked_in != rendered:
                raise FixtureError(
                    f"{args.fixture} is stale; rerun this command with --write"
                )
        digest = hashlib.sha256(rendered).hexdigest()
        print(
            "[swift-offline-device-attestation] "
            f"two-pass fixture check passed sha256={digest}"
        )
        return 0
    except FixtureError as error:
        print(f"swift offline-device-attestation fixture check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
