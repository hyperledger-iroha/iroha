#!/usr/bin/env python3
"""Retain bounded TLC inputs and raw process evidence using Python's stdlib.

The two multilane shell runners own their result assertions; this recorder also
requires empty TLC stderr before recording their acceptance. It never
interprets a counterexample as a proof or release receipt. Each invocation and
case is fresh; failed and interrupted runs are retained. The parent directory
must already exist (the CI formal evidence directory, or the system temp dir).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import stat
import subprocess
import sys
import tempfile
import time


SCHEMA = "sumeragi-v2-bounded-tlc-artifacts-v1"


def digest(path: Path) -> dict:
    """Hash one regular, non-symlink file without loading it all into memory."""
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected regular non-symlink file: {path}")
    before = path.stat()
    hasher = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            hasher.update(chunk)
    after = path.stat()
    identity = lambda value: (value.st_dev, value.st_ino, value.st_mode,
                              value.st_size, value.st_mtime_ns, value.st_ctime_ns)
    if identity(before) != identity(after):
        raise ValueError(f"file changed while hashing: {path}")
    return {"sha256": hasher.hexdigest(), "bytes": after.st_size}


def write_json(path: Path, value: dict) -> None:
    """Create one record, never replacing an earlier observation."""
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, sort_keys=True, indent=2)
        stream.write("\n")


def read_json(path: Path) -> dict:
    digest(path)
    return json.loads(path.read_text(encoding="utf-8"))


def snapshot(source: Path, destination: Path) -> dict:
    """Copy exact bytes and reject a source that changes during the copy."""
    expected = digest(source)
    with source.open("rb") as incoming, destination.open("xb") as outgoing:
        for chunk in iter(lambda: incoming.read(1024 * 1024), b""):
            outgoing.write(chunk)
    if digest(destination) != expected or digest(source) != expected:
        raise ValueError(f"input changed while copying: {source}")
    return {"source": str(source), **expected}


def run_directory(value: str) -> Path:
    path = Path(value)
    if path.is_symlink() or not path.is_dir():
        raise ValueError(f"expected private invocation directory: {path}")
    if stat.S_IMODE(path.stat().st_mode) != 0o700:
        raise ValueError(f"invocation directory must have mode 0700: {path}")
    return path.resolve(strict=True)


def case_directory(root: Path, name: str) -> Path:
    if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", name) is None:
        raise ValueError(f"invalid case name: {name!r}")
    return root / "cases" / name


def initialize(args: argparse.Namespace) -> None:
    parent = args.parent.resolve(strict=True)
    if not parent.is_dir() or args.expected_cases <= 0:
        raise ValueError("existing artifact parent and positive expected case count required")
    root = Path(tempfile.mkdtemp(prefix="sumeragi-v2-tlc-", dir=parent))
    # Announce immediately, including when a subsequent setup operation fails.
    print(f"[tlc] retained artifacts: {root}", file=sys.stderr, flush=True)
    (root / "support").mkdir(mode=0o700)
    (root / "cases").mkdir(mode=0o700)
    support = {}
    paths = [Path(args.runner), Path(__file__), *map(Path, args.support)]
    for path in paths:
        source = path.resolve(strict=True)
        support[source.name] = snapshot(source, root / "support" / source.name)
    write_json(root / "invocation.json", {
        "schema": SCHEMA,
        "claim": "bounded abstract model evidence; no release or refinement receipt",
        "expected_cases": args.expected_cases,
        "started_unix_ns": time.time_ns(),
        "support": support,
        "support_execution": {
            "runner_and_result_checker": "original repository paths; copies are provenance snapshots",
            "java_resolver_and_structural_preflight": "original repository paths",
            "artifact_recorder": "retained support copy",
        },
    })
    print(root, flush=True)


def prepare_tools(args: argparse.Namespace) -> None:
    root = run_directory(args.run_dir)
    tools = root / "tools"
    tools.mkdir(mode=0o700)
    jar = snapshot(Path(args.jar).resolve(strict=True), tools / "tla2tools.jar")
    if jar["sha256"] != args.jar_sha256:
        raise ValueError("retained TLA2Tools jar does not match the pinned checksum")
    java = Path(args.java).resolve(strict=True)
    java_digest = digest(java)
    version_command = [str(java), "-version"]
    version = subprocess.run(version_command, stdin=subprocess.DEVNULL,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    for name, data in (("java-version.stdout.log", version.stdout),
                       ("java-version.stderr.log", version.stderr)):
        with (tools / name).open("xb") as stream:
            stream.write(data)
    write_json(root / "tools.json", {
        "java": {"source": str(java), **java_digest},
        "jar": jar,
        "python": {"source": sys.executable, **digest(Path(sys.executable).resolve())},
        "java_version": {
            "command": version_command,
            "returncode": version.returncode,
            "raw_artifacts": {name: digest(tools / name) for name in (
                "java-version.stdout.log", "java-version.stderr.log")},
        },
    })
    if version.returncode != 0 or digest(java) != java_digest:
        raise ValueError("recorded Java failed its version check or changed during setup")


def check_tools(root: Path) -> dict:
    tools = read_json(root / "tools.json")
    for record, path in (
        (tools["java"], Path(tools["java"]["source"])),
        (tools["jar"], root / "tools" / "tla2tools.jar"),
    ):
        if digest(path) != {key: record[key] for key in ("sha256", "bytes")}:
            raise ValueError(f"recorded tool changed: {path}")
    for name, expected in tools["java_version"]["raw_artifacts"].items():
        if digest(root / "tools" / name) != expected:
            raise ValueError(f"recorded Java version output changed: {name}")
    return tools


def capture(args: argparse.Namespace) -> int:
    root = run_directory(args.run_dir)
    tools = check_tools(root)
    case = case_directory(root, args.name)
    case.mkdir(mode=0o700)
    inputs = case / "inputs"
    inputs.mkdir(mode=0o700)
    snapshots = {}
    for name in (args.module, args.config):
        if Path(name).name != name or name in (".", ".."):
            raise ValueError(f"input must be a basename: {name!r}")
        snapshots[name] = snapshot(Path(args.formal_dir) / name, inputs / name)
    command = args.command
    if command[:1] == ["--"]:
        command = command[1:]
    if command[:4] != [tools["java"]["source"], "-XX:+UseParallelGC", "-cp",
                        str(root / "tools" / "tla2tools.jar")]:
        raise ValueError("command must use the recorded Java and copied TLA2Tools jar")
    if command[-3:] != ["-config", args.config, args.module]:
        raise ValueError("command must select the retained module and config")
    write_json(case / "started.json", {
        "schema": SCHEMA,
        "name": args.name,
        "expectation": args.expectation,
        "command": command,
        "cwd": str(inputs),
        "inputs": snapshots,
        "tools_sha256": digest(root / "tools.json")["sha256"],
        "invocation_sha256": digest(root / "invocation.json")["sha256"],
        "started_unix_ns": time.time_ns(),
    })
    # Keep both original streams. The combined log preserves observed chunk
    # order for convenient inspection; it does not claim a total order
    # between independent stdout/stderr writes in the child.
    with (case / "stdout.log").open("xb", buffering=0) as stdout, \
            (case / "stderr.log").open("xb", buffering=0) as stderr, \
            (case / "combined.log").open("xb", buffering=0) as combined:
        process = subprocess.Popen(command, cwd=inputs, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ, stdout)
            selector.register(process.stderr, selectors.EVENT_READ, stderr)
            while selector.get_map():
                for key, _ in selector.select():
                    chunk = os.read(key.fileobj.fileno(), 65536)
                    if chunk:
                        key.data.write(chunk)
                        combined.write(chunk)
                    else:
                        selector.unregister(key.fileobj)
                        key.fileobj.close()
        status = process.wait()
    write_json(case / "result.json", {
        "schema": SCHEMA,
        "returncode": status,
        "finished_unix_ns": time.time_ns(),
        "started_sha256": digest(case / "started.json")["sha256"],
        "raw_artifacts": {name: digest(case / name) for name in (
            "stdout.log", "stderr.log", "combined.log")},
        "counterexample_location": "stdout.log (unmodified raw TLC transcript)",
    })
    check_tools(root)
    for name, record in snapshots.items():
        if digest(inputs / name) != {key: record[key] for key in ("sha256", "bytes")}:
            raise ValueError(f"executed input changed: {name}")
    return status if status >= 0 else 128 - status


def validate_case(root: Path, case: Path, *, accepted: bool = False) -> dict:
    """Recheck retained hash links at both acceptance and final completion."""
    result = read_json(case / "result.json")
    started = read_json(case / "started.json")
    for expected, path in (
        (result["started_sha256"], case / "started.json"),
        (started["invocation_sha256"], root / "invocation.json"),
        (started["tools_sha256"], root / "tools.json"),
    ):
        if digest(path)["sha256"] != expected:
            raise ValueError(f"retained record hash changed: {path}")
    invocation = read_json(root / "invocation.json")
    for name, record in invocation["support"].items():
        if digest(root / "support" / name) != {
            key: record[key] for key in ("sha256", "bytes")
        }:
            raise ValueError(f"retained support snapshot changed: {name}")
    check_tools(root)
    for name, record in started["inputs"].items():
        if digest(case / "inputs" / name) != {
            key: record[key] for key in ("sha256", "bytes")
        }:
            raise ValueError(f"retained executed input changed: {name}")
    expected_status = 0 if started["expectation"] == "fixed-success" else 12
    if result["returncode"] != expected_status:
        raise ValueError("cannot accept an unexpected process status")
    for name, expected in result["raw_artifacts"].items():
        if digest(case / name) != expected:
            raise ValueError(f"raw artifact changed before acceptance: {name}")
    if accepted:
        marker = read_json(case / "accepted.json")
        if marker["result_sha256"] != digest(case / "result.json")["sha256"]:
            raise ValueError(f"accepted result changed: {case.name}")
        if marker["stderr_empty"] is not True or result["raw_artifacts"]["stderr.log"]["bytes"]:
            raise ValueError(f"accepted case has nonempty stderr: {case.name}")
    return result


def accept(args: argparse.Namespace) -> None:
    root = run_directory(args.run_dir)
    case = case_directory(root, args.name)
    result = validate_case(root, case)
    # Separate streams are authoritative. A convenience merged view cannot
    # establish that stderr was written before the final stdout footer.
    if result["raw_artifacts"]["stderr.log"]["bytes"] != 0:
        write_json(case / "rejected.json", {
            "schema": SCHEMA,
            "reason": "TLC stderr must be empty",
            "result_sha256": digest(case / "result.json")["sha256"],
        })
        raise ValueError("TLC stderr must be empty; raw stderr and rejection retained")
    write_json(case / "accepted.json", {
        "schema": SCHEMA,
        "acceptance": "all existing shell result assertions passed",
        "stderr_empty": True,
        "result_sha256": digest(case / "result.json")["sha256"],
    })


def finish(args: argparse.Namespace) -> None:
    root = run_directory(args.run_dir)
    invocation = read_json(root / "invocation.json")
    accepted = {}
    consistency_errors = []
    for case in sorted((root / "cases").iterdir()):
        marker = case / "accepted.json"
        if marker.exists():
            accepted[case.name] = digest(marker)
        if args.status == 0:
            try:
                validate_case(root, case, accepted=True)
            except (OSError, ValueError, KeyError) as error:
                consistency_errors.append(f"{case.name}: {error}")
    complete = (args.status == 0 and not consistency_errors
                and len(accepted) == invocation["expected_cases"])
    write_json(root / "finished.json", {
        "schema": SCHEMA,
        "runner_body_exit_status": args.status,
        "runner_exit_status": args.status if args.status != 0 or complete else 1,
        "all_expected_cases_accepted": complete,
        "accepted_cases": accepted,
        "consistency_errors": consistency_errors,
        "finished_unix_ns": time.time_ns(),
    })
    if args.status == 0 and not complete:
        raise ValueError("successful runner lacks consistent acceptance for every expected case")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)
    init = subparsers.add_parser("init", help="create and announce a fresh retained invocation")
    init.add_argument("--parent", required=True, type=Path)
    init.add_argument("--runner", required=True)
    init.add_argument("--expected-cases", required=True, type=int)
    init.add_argument("--support", action="append", default=[])
    init.set_defaults(function=initialize)
    tool = subparsers.add_parser("tools", help="snapshot the pinned jar and fingerprint Java")
    tool.add_argument("--java", required=True)
    tool.add_argument("--jar", required=True)
    tool.add_argument("--jar-sha256", required=True)
    tool.set_defaults(function=prepare_tools)
    run = subparsers.add_parser("capture", help="retain and execute one case")
    run.add_argument("--name", required=True)
    run.add_argument("--formal-dir", required=True)
    run.add_argument("--module", required=True)
    run.add_argument("--config", required=True)
    run.add_argument("--expectation", required=True)
    run.add_argument("command", nargs=argparse.REMAINDER)
    run.set_defaults(function=capture)
    accepted = subparsers.add_parser("accept", help="record shell acceptance after its assertions")
    accepted.add_argument("--name", required=True)
    accepted.set_defaults(function=accept)
    done = subparsers.add_parser("finish", help="retain terminal runner status, including failures")
    done.add_argument("--status", required=True, type=int)
    done.set_defaults(function=finish)
    for command in (tool, run, accepted, done):
        command.add_argument("--run-dir", required=True)
    args = parser.parse_args()
    os.umask(0o077)
    try:
        return args.function(args) or 0
    except (OSError, ValueError, KeyError) as error:
        if args.action == "capture":
            case = case_directory(Path(args.run_dir), args.name)
            if case.is_dir() and not (case / "rejected.json").exists():
                write_json(case / "rejected.json", {
                    "schema": SCHEMA,
                    "reason": str(error),
                    "stage": "capture",
                    "recorder_exit_status": 125,
                })
        print(f"TLC artifact retention failed: {error}", file=sys.stderr)
        return 125


if __name__ == "__main__":
    sys.exit(main())
