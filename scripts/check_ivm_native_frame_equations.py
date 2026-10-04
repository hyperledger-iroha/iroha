#!/usr/bin/env python3
"""Run the native IVM frame producers and their four AIR equation consumers.

Requires Python 3.10+ and explicit, already-built ivm/iroha_core_privacy libtest
executables from this checkout. No Cargo, credentials, or inherited capture paths
are needed. --output-dir must be an absent descendant of this checkout's target
directory. All outputs are create-only and enclosed in an owner-only directory.

The receipt identifies executed binary bytes, not their source/build provenance.
These diagnostic captures exercise native/component equivalence; they do not
establish original instruction-packet ownership or a complete invocation proof.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import selectors
import subprocess
import sys
import time

from release_artifact_contract import (
    ReleaseArtifactError,
    StableFile,
    canonical_json_bytes,
    create_fresh_directory,
    ensure_private_directory,
    exclusive_output_fd,
    exclusive_write_bytes,
    stable_hash_path,
    stable_open_relative,
)


ROOT = Path(__file__).resolve().parents[1]
MAX_BINARY_BYTES = 512 * 1024 * 1024
MAX_LOG_BYTES = 20 * 1024 * 1024
MAX_RECEIPT_BYTES = 64 * 1024
PRODUCERS = (
    (
        "call_frame::native_equation_fixture_tests::capture_actual_native_frame_owner_equations",
        "IVM_NATIVE_FRAME_OWNER_CAPTURE=",
        "ivm.native-frame-owner-equations.v1",
        "native-frame-owner",
        "IROHA_IVM_NATIVE_FRAME_OWNER_CAPTURE",
        16 * 1024 * 1024,
    ),
    (
        "ivm::call_runtime::equation_fixture_tests::capture_native_runtime_frame_equations",
        "IVM_NATIVE_CALL_RUNTIME_CAPTURE=",
        "ivm.native-call-runtime-equations.v1",
        "native-call-runtime",
        "IROHA_IVM_NATIVE_CALL_RUNTIME_CAPTURE",
        4 * 1024 * 1024,
    ),
)
CONSUMERS = tuple(
    "execution_proofs::ivm_step_air::machine_bus::" + name
    for name in (
        "frame_descriptor::tests::genuine_native_call_runtime_entries_match_original_operands_and_installed_descriptors",
        "frame_descriptor::tests::genuine_native_frame_owner_entries_match_all_original_descriptor_fields",
        "return_copyback::tests::genuine_native_call_runtime_return_operands_bind_protected_state_and_program_metadata",
        "return_copyback::tests::genuine_native_initialization_and_copyback_match_every_mandatory_cell",
    )
)


def repository_path(raw: Path, *, output: bool = False) -> Path:
    """Check lexical containment without resolving away symlink components."""
    path = raw if raw.is_absolute() else ROOT / raw
    base = ROOT / "target" if output else ROOT
    if ".." in path.parts or not path.is_relative_to(base) or path == base:
        raise ReleaseArtifactError(f"path must be a strict descendant of {base}")
    return path


def pin_binary(source: Path, destination: Path) -> tuple[StableFile, StableFile]:
    """Copy a stable executable through pinned descriptors, with bounded memory."""
    original = stable_hash_path(source, max_size=MAX_BINARY_BYTES)
    if not original.mode & 0o111:
        raise ReleaseArtifactError(f"test binary is not executable: {source}")
    with stable_open_relative(source.parent, source.name, expected=original) as src:
        # The enclosing 0700 directory makes this copy owner-accessible only.
        with exclusive_output_fd(destination, mode=0o755) as dst:
            remaining = original.size
            while remaining:
                chunk = os.read(src, min(remaining, 1024 * 1024))
                if not chunk:
                    raise ReleaseArtifactError("test binary shortened during copying")
                remaining -= len(chunk)
                view = memoryview(chunk)
                while view:
                    written = os.write(dst, view)
                    if written <= 0:
                        raise ReleaseArtifactError("short pinned executable write")
                    view = view[written:]
    pinned = stable_hash_path(destination, max_size=MAX_BINARY_BYTES)
    if (pinned.sha256, pinned.size) != (original.sha256, original.size):
        raise ReleaseArtifactError("pinned executable differs from the requested binary")
    return original, pinned


def capture_test(
    binary: Path, test: str, environment: dict[str, str], timeout: int
) -> tuple[int, bytes]:
    """Capture one exact test, bounding elapsed time and combined output bytes."""
    command = [str(binary), "--exact", test, "--ignored", "--nocapture", "--test-threads=1"]
    deadline = time.monotonic() + timeout
    output = bytearray()
    with subprocess.Popen(
        command, cwd=ROOT, env=environment, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
    ) as child:
        assert child.stdout is not None
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(child.stdout, selectors.EVENT_READ)
                while selector.get_map():
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise ReleaseArtifactError(f"test exceeded {timeout}s: {test}")
                    for key, _ in selector.select(min(remaining, 0.25)):
                        chunk = os.read(key.fd, 64 * 1024)
                        if not chunk:
                            selector.unregister(key.fileobj)
                        elif len(output) + len(chunk) > MAX_LOG_BYTES:
                            raise ReleaseArtifactError(f"test output exceeds byte limit: {test}")
                        else:
                            output.extend(chunk)
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(command, timeout)
            return child.wait(timeout=remaining), bytes(output)
        except subprocess.TimeoutExpired as exc:
            raise ReleaseArtifactError(f"test exceeded {timeout}s: {test}") from exc
        finally:
            # Only this runner's own test subprocess can be stopped here.
            if child.poll() is None:
                child.kill()
                child.wait()


def require_one_pass(code: int, log: bytes, test: str) -> None:
    """Reject skipped, missing, duplicate, failed, or unexpected libtest runs."""
    text = log.decode("utf-8", errors="strict")
    summaries = re.findall(r"^test result: .+$", text, re.MULTILINE)
    expected = r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9.]+s"
    starts = re.findall(r"^test ([^\r\n ]+) \.\.\. ", text, re.MULTILINE)
    if (
        code != 0
        or re.findall(r"^running [0-9]+ tests?$", text, re.MULTILINE) != ["running 1 test"]
        or starts != [test]
        or len(summaries) != 1
        or re.fullmatch(expected, summaries[0]) is None
    ):
        raise ReleaseArtifactError(f"expected exactly one passing test (exit {code}): {test}")


def _unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ReleaseArtifactError("capture JSON contains a duplicate key")
        value[key] = item
    return value


def _reject_float(_raw: str) -> object:
    raise ReleaseArtifactError("capture JSON must use finite integer numbers")


def extract_capture(log: bytes, marker: str, schema: str, limit: int) -> bytes:
    """Validate the actual JSON envelope; Rust consumers own equation checks."""
    prefix = marker.encode("ascii")
    if log.count(prefix) != 1:
        raise ReleaseArtifactError(f"expected exactly one capture marker: {marker}")
    payload = log.split(prefix, 1)[1].split(b"\n", 1)[0]
    if not payload or len(payload) > limit:
        raise ReleaseArtifactError("capture JSON is empty or exceeds its byte limit")
    try:
        value = json.loads(
            payload.decode("utf-8"), object_pairs_hook=_unique_object,
            parse_constant=_reject_float, parse_float=_reject_float,
        )
    except (ValueError, UnicodeError, RecursionError) as exc:
        raise ReleaseArtifactError("capture is not bounded valid UTF-8 JSON") from exc
    if not isinstance(value, dict) or value.get("schema") != schema:
        raise ReleaseArtifactError("capture JSON has the wrong schema")
    if schema == PRODUCERS[0][2]:
        if set(value) != {"schema", "cells", "cases"} or value["cells"] != 4097:
            raise ReleaseArtifactError("native owner capture envelope is malformed")
        cases = value["cases"]
        if not isinstance(cases, list) or len(cases) != 12:
            raise ReleaseArtifactError("native owner capture must contain all twelve cases")
        geometries = []
        for case in cases:
            if not isinstance(case, dict) or any(
                type(case.get(name)) is not kind
                for name, kind in (("result_words", int), ("shift", int), ("root_return", bool))
            ):
                raise ReleaseArtifactError("native owner capture case geometry is malformed")
            geometries.append((case["result_words"], case["shift"], case["root_return"]))
        if sorted(geometries) != sorted(
            (words, shift, root) for words in (1, 2, 8192) for shift in (0, 8) for root in (True, False)
        ):
            raise ReleaseArtifactError("native owner capture case geometry is incomplete")
    else:
        if (
            type(value.get("native_program_result")) is not int
            or value["native_program_result"] != 1
            or not isinstance(value.get("program_bytes"), list)
            or not value["program_bytes"]
            or any(type(byte) is not int or not 0 <= byte <= 255 for byte in value["program_bytes"])
            or any(not isinstance(value.get(name), dict) for name in (
                "root_entry", "child_entry", "child_return", "root_return",
            ))
        ):
            raise ReleaseArtifactError("native call-runtime capture envelope is malformed")
    return payload + b"\n"


def descriptor(path: Path, info: StableFile, output: Path) -> dict[str, object]:
    """Describe one retained output without including its potentially large data."""
    return {"path": str(path.relative_to(output)), "sha256": info.sha256, "size": info.size}


class LaunchPath:
    """Retain the repository-to-output directory chain across all child runs."""

    def __init__(self, path: Path):
        self.entries: list[tuple[str, int, int | None]] = []
        flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
        try:
            descriptor_fd = os.open(ROOT, flags)
            self.entries.append((str(ROOT), descriptor_fd, None))
            for component in path.relative_to(ROOT).parts:
                parent_fd = descriptor_fd
                descriptor_fd = os.open(component, flags, dir_fd=parent_fd)
                self.entries.append((component, descriptor_fd, parent_fd))
            self.inspect()
        except BaseException:
            self.close()
            raise

    def close(self) -> None:
        for _, descriptor_fd, _ in reversed(self.entries):
            os.close(descriptor_fd)
        self.entries.clear()

    def inspect(self) -> list[os.stat_result]:
        states = []
        for name, descriptor_fd, parent_fd in self.entries:
            opened = os.fstat(descriptor_fd)
            named = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
            if (opened.st_dev, opened.st_ino) != (named.st_dev, named.st_ino):
                raise ReleaseArtifactError("validation launch-path directory was replaced")
            states.append(opened)
        return states

    @contextmanager
    def unchanged(self):
        """Reject rename/restore of any launch ancestor as well as the leaf.

        Directory change timestamps intentionally make concurrent namespace
        changes a local retry. This is ordinary local filesystem custody, not
        attestation against an executor capable of forging kernel metadata.
        """
        before = self.inspect()
        yield
        after = self.inspect()
        fields = ("st_dev", "st_ino", "st_uid", "st_mode", "st_nlink", "st_mtime_ns", "st_ctime_ns")
        if any(getattr(original, field) != getattr(current, field)
               for original, current in zip(before, after) for field in fields):
            raise ReleaseArtifactError("validation launch-path directory changed during launch")


@contextmanager
def retained_launch_path(path: Path):
    owner = LaunchPath(path)
    try:
        yield owner
    finally:
        owner.close()


def validate(ivm: Path, privacy: Path, output: Path, timeout: int) -> Path:
    """Run fresh producers and exact consumers, publishing the receipt last."""
    ivm, privacy = repository_path(ivm), repository_path(privacy)
    output = repository_path(output, output=True)
    create_fresh_directory(output, mode=0o700)
    ensure_private_directory(output, anchor=output.parent)
    with retained_launch_path(output) as launch_path:
        return validate_retained(ivm, privacy, output, timeout, launch_path)


def validate_retained(ivm: Path, privacy: Path, output: Path, timeout: int, launch_path: LaunchPath) -> Path:
    """Run with the original directory descriptors retained through publication."""
    environment = os.environ.copy()
    for producer in PRODUCERS:
        environment.pop(producer[4], None)
    binaries = []
    for label, source in (("ivm", ivm), ("privacy", privacy)):
        pinned_path = output / f"{label}-test-binary"
        original, pinned = pin_binary(source, pinned_path)
        binaries.append((source, original, pinned_path, pinned))

    retained: list[tuple[Path, StableFile]] = []
    tests: list[dict[str, object]] = []
    captures: list[dict[str, object]] = []

    def run(binary: Path, name: str, stem: str) -> bytes:
        ensure_private_directory(output, anchor=output.parent)
        with launch_path.unchanged():
            code, log = capture_test(binary, name, environment, timeout)
        log_path = output / f"{stem}.log"
        exclusive_write_bytes(log_path, log, mode=0o600)
        require_one_pass(code, log, name)
        info = stable_hash_path(log_path, max_size=MAX_LOG_BYTES)
        retained.append((log_path, info))
        tests.append({"test": name, "passed": 1, "failed": 0, "ignored": 0,
                      "exit_code": code, "log": descriptor(log_path, info, output)})
        return log

    for name, marker, schema, stem, variable, limit in PRODUCERS:
        log = run(binaries[0][2], name, f"{stem}-producer")
        payload = extract_capture(log, marker, schema, limit)
        path = output / f"{stem}-capture.json"
        exclusive_write_bytes(path, payload, mode=0o600)
        info = stable_hash_path(path, max_size=limit + 1)
        retained.append((path, info))
        captures.append({"schema": schema, **descriptor(path, info, output)})
        environment[variable] = str(path)
    for index, name in enumerate(CONSUMERS, start=1):
        run(binaries[1][2], name, f"consumer-{index}")

    launch_path.inspect()
    for path, before in retained + [
        (path, info) for source, original, pinned, identity in binaries
        for path, info in ((source, original), (pinned, identity))
    ]:
        if stable_hash_path(path, max_size=max(MAX_BINARY_BYTES, MAX_LOG_BYTES)) != before:
            raise ReleaseArtifactError(f"validation input/output changed during execution: {path}")
    receipt = {
        "schema": "ivm.native-frame-equation-validation.v1",
        "status": "passed",
        "scope": "Native frame/call-method and AIR component equivalence diagnostics only.",
        "source_build_provenance": "unverified; binary paths and hashes do not establish a common source candidate",
        "complete_invocation_proof": False,
        "binaries": [{"source_path": str(source.relative_to(ROOT)),
                      **descriptor(pinned, identity, output)}
                     for source, _, pinned, identity in binaries],
        "captures": captures,
        "tests": tests,
        "totals": {"producers_passed": 2, "consumers_passed": 4, "failed": 0, "ignored": 0},
    }
    payload = canonical_json_bytes(receipt)
    if len(payload) > MAX_RECEIPT_BYTES:
        raise ReleaseArtifactError("validation receipt exceeds its byte limit")
    ensure_private_directory(output, anchor=output.parent)
    launch_path.inspect()
    path = output / "receipt.json"
    with exclusive_output_fd(path, mode=0o600) as fd:
        # Revalidate the retained chain after the helper has pinned the actual
        # publication parent. Failure here makes its create-only owner remove
        # the incomplete output rather than retain a successful-looking receipt.
        launch_path.inspect()
        remaining = memoryview(payload)
        while remaining:
            written = os.write(fd, remaining)
            if written <= 0:
                raise ReleaseArtifactError("short validation receipt write")
            remaining = remaining[written:]
        launch_path.inspect()
    launch_path.inspect()
    return path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ivm-test-binary", type=Path, required=True)
    parser.add_argument("--privacy-test-binary", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True, help="absent directory below repository target/")
    parser.add_argument("--test-timeout-seconds", type=int, default=1200, help="per-test timeout (1..1200; default 1200)")
    args = parser.parse_args()
    if not 1 <= args.test_timeout_seconds <= 1200:
        parser.error("--test-timeout-seconds must be in 1..1200")
    try:
        path = validate(args.ivm_test_binary, args.privacy_test_binary, args.output_dir, args.test_timeout_seconds)
    except (ReleaseArtifactError, OSError, UnicodeError) as exc:
        print(f"native frame validation failed: {exc}", file=sys.stderr)
        return 1
    print(f"Native producers 2/2 and equation consumers 4/4 passed; receipt: {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
