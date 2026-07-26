#!/usr/bin/env python3
"""Regenerate the exact-current Rust/Kotodama admission parity fixture.

Prerequisites:

* a freshly built ``koto`` executable;
* the matching ``libivm-*.rlib`` and its dependency directory;
* ``rustc`` from the toolchain that built the rlib.

The explicit tool inputs are content-addressed in the generated fixture. Relative
paths are resolved from the repository root. The script never invokes Cargo and
does not modify ``Cargo.lock``.
"""

from __future__ import annotations

import argparse
import base64
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import Any, Sequence


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
FIXTURE_DIRECTORY = Path("javascript/iroha_js/test/fixtures")
SOURCE_PATH = FIXTURE_DIRECTORY / "current_rust_contract_artifact.ko"
FIXTURE_PATH = FIXTURE_DIRECTORY / "current_rust_contract_artifact.json"
VERIFIER_PATH = FIXTURE_DIRECTORY / "verify_current_rust_contract_artifact.rs"
GENERATOR_PATH = Path("scripts/regenerate_current_rust_contract_artifact.py")
SOURCE_BINDINGS = (
    (SOURCE_PATH, "contract_source_git_blob"),
    (GENERATOR_PATH, "artifact_generator_git_blob"),
    (VERIFIER_PATH, "rust_verifier_rs_git_blob"),
    (Path("crates/ivm/src/contract_artifact.rs"), "contract_artifact_rs_git_blob"),
    (Path("crates/ivm_abi/src/syscalls.rs"), "ivm_syscalls_rs_git_blob"),
    (Path("crates/kotodama_lang/src/compiler.rs"), "kotodama_compiler_rs_git_blob"),
    (Path("Cargo.lock"), "cargo_lock_git_blob"),
)
VERIFIER_FIELDS = (
    "code_hash_hex",
    "abi_hash_hex",
    "header_len",
    "code_offset",
    "entrypoint_count",
)
HASH_LITERAL = re.compile(r"^hash:([0-9A-F]{64})#[0-9A-F]{4}$")
LOWER_HEX_32 = re.compile(r"^[0-9a-f]{64}$")


class FixtureError(RuntimeError):
    """Raised when fixture generation cannot prove exact-current parity."""


def _read(path: Path) -> bytes:
    try:
        return path.read_bytes()
    except OSError as error:
        raise FixtureError(f"failed to read {path}: {error}") from error


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _git_blob_id(data: bytes) -> str:
    header = f"blob {len(data)}\0".encode()
    return hashlib.sha1(header + data).hexdigest()


def _resolve_file(raw: Path, name: str, *, executable: bool = False) -> Path:
    path = raw if raw.is_absolute() else REPOSITORY_ROOT / raw
    path = path.resolve()
    if not path.is_file():
        raise FixtureError(f"{name} is missing or is not a file: {path}")
    if executable and not os.access(path, os.X_OK):
        raise FixtureError(f"{name} is not executable: {path}")
    return path


def _resolve_rustc(raw: str) -> Path:
    if os.sep in raw or (os.altsep is not None and os.altsep in raw):
        candidate = Path(raw)
        candidate = candidate if candidate.is_absolute() else REPOSITORY_ROOT / candidate
        candidate = candidate.absolute()
        if not candidate.is_file() or not os.access(candidate, os.X_OK):
            raise FixtureError(f"rustc is missing or is not executable: {candidate}")
        # Do not resolve the rustup proxy symlink: rustup dispatches from argv[0].
        return candidate
    located = shutil.which(raw)
    if located is None:
        raise FixtureError(f"rustc command was not found: {raw}")
    return Path(located).absolute()


def _run(command: Sequence[os.PathLike[str] | str]) -> subprocess.CompletedProcess[str]:
    rendered = [os.fspath(argument) for argument in command]
    try:
        return subprocess.run(
            rendered,
            cwd=REPOSITORY_ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except subprocess.CalledProcessError as error:
        detail = error.stderr.strip() or error.stdout.strip() or "no diagnostic output"
        raise FixtureError(
            f"command failed ({error.returncode}): {' '.join(rendered)}\n{detail}"
        ) from error
    except OSError as error:
        raise FixtureError(f"failed to run {' '.join(rendered)}: {error}") from error


def _build_artifact(koto: Path, stage: Path) -> tuple[bytes, dict[str, Any]]:
    source = REPOSITORY_ROOT / SOURCE_PATH
    _run((koto, "fmt", "--check", source))

    artifact_path = stage / "current_rust_contract_artifact.to"
    manifest_path = stage / "current_rust_contract_artifact.manifest.json"
    _run(
        (
            koto,
            "build",
            "--profile",
            "release",
            "--target-dir",
            stage / "kotodama-target",
            "--out",
            artifact_path,
            "--manifest-out",
            manifest_path,
            source,
        )
    )
    artifact = _read(artifact_path)
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise FixtureError(f"failed to parse generated manifest {manifest_path}: {error}") from error
    if not isinstance(manifest, dict):
        raise FixtureError("generated compiler manifest is not a JSON object")
    return artifact, manifest


def _rust_verifier(
    rustc: Path, ivm_rlib: Path, artifact_path: Path, stage: Path
) -> dict[str, str | int]:
    executable = stage / "verify-current-rust-contract-artifact"
    _run(
        (
            rustc,
            "--edition=2024",
            REPOSITORY_ROOT / VERIFIER_PATH,
            "-L",
            f"dependency={ivm_rlib.parent}",
            "--extern",
            f"ivm={ivm_rlib}",
            "-o",
            executable,
        )
    )
    output = _run((executable, artifact_path)).stdout
    values: dict[str, str] = {}
    for line in output.splitlines():
        field, separator, value = line.partition("=")
        if not separator or field not in VERIFIER_FIELDS or field in values:
            raise FixtureError(f"Rust verifier emitted a noncanonical line: {line!r}")
        values[field] = value
    if set(values) != set(VERIFIER_FIELDS):
        missing = ", ".join(sorted(set(VERIFIER_FIELDS) - set(values)))
        raise FixtureError(f"Rust verifier omitted fields: {missing}")
    for field in ("code_hash_hex", "abi_hash_hex"):
        if LOWER_HEX_32.fullmatch(values[field]) is None:
            raise FixtureError(f"Rust verifier emitted invalid {field}: {values[field]!r}")
    result: dict[str, str | int] = {
        "code_hash_hex": values["code_hash_hex"],
        "abi_hash_hex": values["abi_hash_hex"],
    }
    for field in ("header_len", "code_offset", "entrypoint_count"):
        try:
            parsed = int(values[field], 10)
        except ValueError as error:
            raise FixtureError(
                f"Rust verifier emitted non-integer {field}: {values[field]!r}"
            ) from error
        if parsed < 0:
            raise FixtureError(f"Rust verifier emitted negative {field}: {parsed}")
        result[field] = parsed
    return result


def _manifest_hash(manifest: dict[str, Any], field: str) -> str:
    value = manifest.get(field)
    if not isinstance(value, str):
        raise FixtureError(f"compiler manifest has no string {field}")
    match = HASH_LITERAL.fullmatch(value)
    if match is None:
        raise FixtureError(f"compiler manifest has noncanonical {field}: {value!r}")
    return match.group(1).lower()


def _provenance(koto: Path, ivm_rlib: Path) -> dict[str, str]:
    result = {
        "koto_sha256": _sha256(_read(koto)),
        "ivm_rlib_sha256": _sha256(_read(ivm_rlib)),
    }
    for relative, field in SOURCE_BINDINGS:
        result[field] = _git_blob_id(_read(REPOSITORY_ROOT / relative))
    return result


def _generate(koto: Path, ivm_rlib: Path, rustc: Path) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="current-rust-contract-artifact.") as raw_stage:
        stage = Path(raw_stage)
        artifact_path = stage / "current_rust_contract_artifact.to"
        artifact, manifest = _build_artifact(koto, stage)
        verifier = _rust_verifier(rustc, ivm_rlib, artifact_path, stage)

    if _manifest_hash(manifest, "code_hash") != verifier["code_hash_hex"]:
        raise FixtureError("compiler manifest and Rust verifier code hashes differ")
    if _manifest_hash(manifest, "abi_hash") != verifier["abi_hash_hex"]:
        raise FixtureError("compiler manifest and Rust verifier ABI hashes differ")
    code_offset = verifier["code_offset"]
    if not isinstance(code_offset, int) or code_offset >= len(artifact):
        raise FixtureError("Rust verifier code offset is outside the generated artifact")

    return {
        "fixture_version": 1,
        "source": SOURCE_PATH.name,
        "artifact_base64": base64.b64encode(artifact).decode("ascii"),
        "artifact_length": len(artifact),
        "artifact_sha256": _sha256(artifact),
        "manifest": manifest,
        "rust_verifier": verifier,
        "generation_provenance": _provenance(koto, ivm_rlib),
    }


def _render(fixture: dict[str, Any]) -> str:
    return json.dumps(fixture, ensure_ascii=False, indent=2) + "\n"


def _atomic_write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        mode = stat.S_IMODE(path.stat().st_mode)
    except FileNotFoundError:
        mode = 0o644
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        os.fchmod(descriptor, mode)
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as output:
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def _check(expected: str, path: Path) -> None:
    try:
        actual = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise FixtureError(f"failed to read checked-in fixture {path}: {error}") from error
    if actual == expected:
        return
    diff = "".join(
        difflib.unified_diff(
            actual.splitlines(keepends=True),
            expected.splitlines(keepends=True),
            fromfile=os.fspath(path),
            tofile=f"{path} (regenerated)",
        )
    )
    raise FixtureError(f"exact-current admission fixture is stale\n{diff}")


def _parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="verify the checked-in JSON")
    mode.add_argument("--write", action="store_true", help="replace the checked-in JSON atomically")
    parser.add_argument("--koto", type=Path, required=True, help="path to the koto executable")
    parser.add_argument(
        "--ivm-rlib",
        type=Path,
        required=True,
        help="path to the matching libivm-*.rlib",
    )
    parser.add_argument(
        "--rustc",
        default="rustc",
        help="rustc command or path (default: rustc from PATH)",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    """Regenerate or check the exact-current admission parity fixture."""

    args = _parse_args(sys.argv[1:] if argv is None else argv)
    try:
        koto = _resolve_file(args.koto, "koto", executable=True)
        ivm_rlib = _resolve_file(args.ivm_rlib, "IVM rlib")
        if not re.fullmatch(r"libivm-[0-9a-f]+\.rlib", ivm_rlib.name):
            raise FixtureError(f"IVM rlib has a noncanonical file name: {ivm_rlib.name}")
        rustc = _resolve_rustc(args.rustc)
        content = _render(_generate(koto, ivm_rlib, rustc))
        fixture_path = REPOSITORY_ROOT / FIXTURE_PATH
        if args.write:
            _atomic_write(fixture_path, content)
            print(f"published {FIXTURE_PATH}")
        else:
            _check(content, fixture_path)
            print(f"verified {FIXTURE_PATH}")
    except FixtureError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
