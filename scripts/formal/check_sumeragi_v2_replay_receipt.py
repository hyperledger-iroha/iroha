#!/usr/bin/env python3
"""Independently verify one Sumeragi V2 replay V1 receipt.

The checker executes no collected tool. It rehashes every source, tool, and
artifact; reconstructs exact invocations and descriptor contracts; derives the
event graph from the collector source with ``ast.literal_eval``; and checks the
normalized bytes against the tracked fixture. V1 is diagnostic-only because
the existing project SSH Git verifier does not sign replay receipts;
``--require-release`` therefore fails closed.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
from typing import Any, Iterable, Union


SCHEMA_NAME = "iroha-sumeragi-v2-replay-receipt-v1"
TLA2TOOLS_SHA256 = "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
TLAPM_HASHES = {
    "tlapm-projection/Folds.tla": "aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da",
    "tlapm-projection/Functions.tla": "b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063",
}
EXPECTED_STATES = 101
EXPECTED_ACTIONS = 100
SEED = 19349663
ARIL = 0
TLC_MAX_SET_SIZE = 1000000
SHA256_RE = re.compile(r"[0-9a-f]{64}", re.ASCII)
CONTROL_RE = re.compile(rb"[\x00-\x08\x0b-\x1f\x7f-\x9f]")
DIAGNOSTIC_RE = re.compile(
    rb"(?im)^[ \t]*(?:error:|warning:|fatal(?: error)?:|exception(?: in thread)?\b|"
    rb"caused by:|suppressed:|deadlock reached(?:\.|$)|temporal properties were violated\.)"
)

BASE_SOURCE_PATHS = {
    "Cargo.lock",
    "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.cfg",
    "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv",
    "scripts/normalize_sumeragi_v2_tlc_trace.py",
    "scripts/formal/check_sumeragi_v2_replay_receipt.py",
    "scripts/formal/check_sumeragi_v2_replay_trace.sh",
    "scripts/formal/collect_sumeragi_v2_replay_receipt.py",
    "scripts/formal/resolve_java.sh",
    "scripts/formal/sumeragi_v2_replay_receipt_v1.schema.json",
    "scripts/formal/sumeragi_v2_tlc_result_contract.sh",
}
TOP_KEYS = {
    "schema", "schema_version", "evidence_class", "mode", "runner", "invocation",
    "source_identity", "tool_identity", "events", "result", "signing",
    "artifact_inventory", "publication",
}
EVENT_KEYS = {
    "name", "argv", "cwd", "environment", "descriptors", "status", "timeout",
    "cleanup", "duration_monotonic_ns", "outputs",
}
FILE_KEYS = {"path", "sha256", "size_bytes", "mode", "nlink"}
class ReceiptError(RuntimeError):
    """Receipt data or one of its bound files is invalid."""


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _absolute(path: Path) -> Path:
    return Path(os.path.abspath(path))


def _require_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ReceiptError(f"{label} fields differ from the V1 contract")
    return value


def _read_file_record(
    path: Path, logical_path: str, *, single_link: bool
) -> tuple[dict[str, Any], bytes]:
    path = _absolute(path)
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{logical_path} is unavailable") from error
    if resolved != path or stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise ReceiptError(f"{logical_path} is not one canonical regular file")
    if single_link and before.st_nlink != 1:
        raise ReceiptError(f"{logical_path} is hard linked")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
        ):
            raise ReceiptError(f"{logical_path} changed while opening")
        digest = hashlib.sha256()
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
            digest.update(chunk)
            total += len(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    stable_opened = (
        opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns,
        opened.st_ctime_ns, stat.S_IMODE(opened.st_mode), opened.st_nlink,
    )
    stable_after = (
        after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns,
        after.st_ctime_ns, stat.S_IMODE(after.st_mode), after.st_nlink,
    )
    if stable_after != stable_opened or total != opened.st_size:
        raise ReceiptError(f"{logical_path} drifted while checking")
    try:
        linked = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{logical_path} changed while checking") from error
    if (
        stat.S_ISLNK(linked.st_mode)
        or (linked.st_dev, linked.st_ino) != (opened.st_dev, opened.st_ino)
    ):
        raise ReceiptError(f"{logical_path} pathname changed while checking")
    return (
        {
            "path": logical_path,
            "sha256": digest.hexdigest(),
            "size_bytes": total,
            "mode": stat.S_IMODE(opened.st_mode),
            "nlink": opened.st_nlink,
        },
        b"".join(chunks),
    )


def _file_record(path: Path, logical_path: str, *, single_link: bool) -> dict[str, Any]:
    record, _ = _read_file_record(path, logical_path, single_link=single_link)
    return record


def _validate_record(record: Any, label: str) -> dict[str, Any]:
    record = _require_keys(record, FILE_KEYS, label)
    if (
        not isinstance(record["path"], str)
        or not isinstance(record["sha256"], str)
        or SHA256_RE.fullmatch(record["sha256"]) is None
        or type(record["size_bytes"]) is not int
        or record["size_bytes"] < 0
        or any(type(record[key]) is not int for key in ("mode", "nlink"))
        or not 0 <= record["mode"] <= 0o7777
        or record["nlink"] < 1
    ):
        raise ReceiptError(f"{label} has invalid file identity values")
    return record


def _manifest(records: list[dict[str, Any]]) -> str:
    return sha256_bytes(canonical_json(sorted(records, key=lambda item: item["path"])))


def _literal_assignment(tree: ast.Module, name: str) -> Any:
    for node in tree.body:
        if isinstance(node, ast.Assign):
            if any(isinstance(target, ast.Name) and target.id == name for target in node.targets):
                return ast.literal_eval(node.value)
    raise ReceiptError(f"collector source does not define literal {name}")


def _source_event_graph(collector: Path, mode: str) -> dict[str, Any]:
    try:
        tree = ast.parse(collector.read_text(encoding="utf-8"), filename=str(collector))
    except (OSError, UnicodeError, SyntaxError) as error:
        raise ReceiptError("collector source cannot be parsed") from error
    templates = list(_literal_assignment(tree, "EVENT_TEMPLATES")[mode])
    return {
        "nodes": [name for name, _ in templates],
        "edges": [
            {"from": dependency, "to": name}
            for name, dependencies in templates
            for dependency in dependencies
        ],
    }


def _tla_dependencies(path: Path) -> list[str]:
    lines = path.read_text(encoding="utf-8").splitlines()
    collected: list[str] = []
    active = False
    for line in lines[:80]:
        if line.startswith("EXTENDS "):
            active = True
            collected.append(line[len("EXTENDS ") :])
        elif active and (line.startswith(" ") or line.startswith("\t")):
            collected.append(line)
        elif active:
            break
    return re.findall(r"[A-Za-z][A-Za-z0-9_]*", " ".join(collected), re.ASCII)


def _tla_closure(root: Path) -> set[str]:
    formal = root / "formal/sumeragi_v2"
    pending = ["SumeragiV2TraceWitness"]
    visited: set[str] = set()
    result: set[str] = set()
    while pending:
        module = pending.pop()
        if module in visited:
            continue
        visited.add(module)
        path = formal / f"{module}.tla"
        if not path.is_file():
            continue
        result.add(str(path.relative_to(root)))
        for dependency in _tla_dependencies(path):
            if (formal / f"{dependency}.tla").is_file():
                pending.append(dependency)
    return result


def _tool_locations(receipt: dict[str, Any], root: Path) -> dict[str, Path]:
    events = {event["name"]: event for event in receipt["events"]}
    sany = events["standalone_sany"]["argv"]
    normalizer = events["normalizer"]["argv"]
    locations = {
        "tool/java": Path(sany[0]),
        "tool/python": Path(normalizer[0]),
        "tool/tla2tools.jar": Path(sany[sany.index("-cp") + 1]),
    }
    library = next(item for item in sany if item.startswith("-DTLA-Library="))
    projection = Path(library.split("=", 1)[1])
    locations["tlapm-projection/Folds.tla"] = projection / "Folds.tla"
    locations["tlapm-projection/Functions.tla"] = projection / "Functions.tla"
    return locations


def _parse_options(argv: list[str]) -> dict[str, str]:
    if len(argv) % 2:
        raise ReceiptError("collector options are not exact pairs")
    result: dict[str, str] = {}
    for index in range(0, len(argv), 2):
        option, value = argv[index : index + 2]
        if not option.startswith("--") or option in result:
            raise ReceiptError("collector options are malformed or duplicated")
        result[option] = value
    return result


def _expected_argv(
    name: str,
    sequence: int,
    root: Path,
    output_root: Path,
    tools: dict[str, Path],
    receipt: dict[str, Any],
) -> Union[list[str], None]:
    formal = root / "formal/sumeragi_v2"
    fixture = root / "crates/iroha_sumeragi_core/tests/fixtures"
    projection = tools["tlapm-projection/Functions.tla"].parent
    java_common = [
        str(tools["tool/java"]), f"-DTLA-Library={projection}", "-cp",
        str(tools["tool/tla2tools.jar"]),
    ]
    if name == "standalone_sany":
        return [*java_common, "tla2sany.SANY", str(formal / "SumeragiV2TraceWitness.tla")]
    if name == "raw_tlc":
        runtime = Path(receipt["invocation"]["runtime_root"])
        return [
            str(tools["tool/java"]), "-XX:+UseParallelGC",
            f"-DTLA-Library={projection}", "-cp", str(tools["tool/tla2tools.jar"]),
            "tlc2.TLC", "-maxSetSize", str(TLC_MAX_SET_SIZE), "-metadir",
            str(runtime / "states"), "-workers", "1", "-depth", "500", "-seed",
            str(SEED), "-aril", str(ARIL), "-simulate", "num=200", "-config",
            str(fixture / "tlc_replay_witness.cfg"), "-tool", "SumeragiV2TraceWitness",
        ]
    if name == "normalizer":
        return [
            str(tools["tool/python"]), "-B", "-I", "-S",
            str(root / "scripts/normalize_sumeragi_v2_tlc_trace.py"),
            str(output_root / "events/02-raw_tlc.stdout"), "--seed", str(SEED),
            "--aril", str(ARIL),
        ]
    return None


def _expected_cwd(name: str, root: Path) -> Path:
    return root / "formal/sumeragi_v2" if name in {"standalone_sany", "raw_tlc"} else root


def _validate_collector_invocation(
    invocation: dict[str, Any],
    receipt: dict[str, Any],
    root: Path,
    output_root: Path,
    tools: dict[str, Path],
) -> None:
    argv = invocation["argv"]
    collector = root / "scripts/formal/collect_sumeragi_v2_replay_receipt.py"
    if not argv or argv[0] != str(collector):
        raise ReceiptError("collector executable path differs")
    options = _parse_options(argv[1:])
    required = {
        "--root", "--java-bin", "--python-bin", "--tla2tools-jar",
        "--tlapm-projection", "--output-root", "--mode", "--timeout-seconds",
    }
    if set(options) != required:
        raise ReceiptError("collector option surface differs")
    expected = {
        "--root": str(root),
        "--java-bin": str(tools["tool/java"]),
        "--python-bin": str(tools["tool/python"]),
        "--tla2tools-jar": str(tools["tool/tla2tools.jar"]),
        "--tlapm-projection": str(tools["tlapm-projection/Functions.tla"].parent),
        "--output-root": str(output_root),
        "--mode": receipt["mode"],
    }
    for option, value in expected.items():
        if options[option] != value:
            raise ReceiptError(f"collector {option} differs")
    try:
        timeout = float(options["--timeout-seconds"])
    except ValueError as error:
        raise ReceiptError("collector timeout option is malformed") from error
    if timeout != invocation["timeout_seconds"]:
        raise ReceiptError("collector timeout argv differs")
    if invocation["cwd"] != str(root):
        raise ReceiptError("collector cwd differs from the canonical worktree")


def _validate_event(
    event: Any,
    sequence: int,
    root: Path,
    output_root: Path,
    tools: dict[str, Path],
    receipt: dict[str, Any],
) -> None:
    event = _require_keys(event, EVENT_KEYS, f"event {sequence}")
    name = event["name"]
    if not isinstance(name, str) or not isinstance(event["argv"], list) or not all(
        isinstance(item, str) for item in event["argv"]
    ):
        raise ReceiptError(f"event {sequence} argv is invalid")
    expected = _expected_argv(name, sequence, root, output_root, tools, receipt)
    if expected is not None and event["argv"] != expected:
        raise ReceiptError(f"{name} argv differs from the V1 runner contract")
    if event["cwd"] != str(_expected_cwd(name, root)):
        raise ReceiptError(f"{name} cwd differs")
    if event["environment"] != receipt["invocation"]["environment"]:
        raise ReceiptError(f"{name} environment differs")
    expected_stdout = f"events/{sequence:02d}-{name}.stdout"
    expected_stderr = f"events/{sequence:02d}-{name}.stderr"
    expected_descriptors = {
        "stdin": {"fd": 0, "kind": "null", "path": "/dev/null"},
        "stdout": {"fd": 1, "kind": "create-only-regular-file", "artifact": expected_stdout},
        "stderr": {"fd": 2, "kind": "create-only-regular-file", "artifact": expected_stderr},
        "close_fds": True,
        "new_session": True,
    }
    if event["descriptors"] != expected_descriptors:
        raise ReceiptError(f"{name} descriptor topology differs")
    expected_status = 12 if name == "raw_tlc" else 0
    if event["status"] != {"actual": expected_status, "expected": expected_status, "matched": True}:
        raise ReceiptError(f"{name} status differs")
    timeout = _require_keys(
        event["timeout"],
        {"seconds", "occurred", "grace_seconds", "sigterm_sent", "sigkill_sent"},
        f"{name} timeout",
    )
    if (
        timeout["seconds"] != receipt["invocation"]["timeout_seconds"]
        or timeout["occurred"] is not False
        or timeout["sigterm_sent"] is not False
        or timeout["sigkill_sent"] is not False
        or timeout["grace_seconds"] != 2.0
    ):
        raise ReceiptError(f"{name} timeout contract differs")
    cleanup = _require_keys(
        event["cleanup"],
        {
            "process_group",
            "scope",
            "post_exit_group_members_observed",
            "process_group_quiescent",
        },
        f"{name} cleanup",
    )
    if (
        type(cleanup["process_group"]) is not int
        or cleanup["process_group"] <= 0
        or cleanup["scope"] != "new-session-process-group"
        or cleanup["post_exit_group_members_observed"] is not False
        or cleanup["process_group_quiescent"] is not True
    ):
        raise ReceiptError(f"{name} process-group cleanup differs")
    try:
        os.killpg(cleanup["process_group"], 0)
    except ProcessLookupError:
        pass
    except PermissionError as error:
        raise ReceiptError(f"{name} process group still exists") from error
    else:
        raise ReceiptError(f"{name} process group still exists")
    if type(event["duration_monotonic_ns"]) is not int or event["duration_monotonic_ns"] < 0:
        raise ReceiptError(f"{name} duration is invalid")
    outputs = _require_keys(event["outputs"], {"stdout", "stderr"}, f"{name} outputs")
    for stream, relative in (("stdout", expected_stdout), ("stderr", expected_stderr)):
        record = _validate_record(outputs[stream], f"{name} {stream}")
        actual, data = _read_file_record(
            output_root / relative, relative, single_link=True
        )
        if record != actual:
            raise ReceiptError(f"{name} {stream} identity differs")
        if stream == "stderr" and data:
            raise ReceiptError(f"{name} separate stderr is not empty")


def _validate_signing(
    receipt: dict[str, Any], require_release: bool
) -> None:
    expected = {
        "status": "unsigned-diagnostic",
        "provider": None,
        "release_evidence": False,
        "attestation": None,
    }
    if receipt["signing"] != expected:
        raise ReceiptError("diagnostic signing record differs")
    if require_release:
        raise ReceiptError(
            "V1 has no project signature over the canonical replay receipt; "
            "diagnostic data is not release evidence"
        )


def _validate_artifact_tree(receipt: dict[str, Any], output_root: Path) -> None:
    inventory = receipt["artifact_inventory"]
    if not isinstance(inventory, list):
        raise ReceiptError("artifact inventory is not a list")
    records = [_validate_record(item, "artifact inventory entry") for item in inventory]
    paths = [item["path"] for item in records]
    if paths != sorted(paths) or len(paths) != len(set(paths)):
        raise ReceiptError("artifact inventory paths are not unique and sorted")
    event_paths = sorted(
        output["path"]
        for event in receipt["events"]
        for output in event["outputs"].values()
    )
    if paths != event_paths:
        raise ReceiptError("artifact inventory does not match exact event outputs")
    expected_files = {"receipt.json", *paths}
    observed_files: set[str] = set()
    observed_directories: set[str] = set()
    for directory, directory_names, file_names in os.walk(output_root, followlinks=False):
        directory_path = Path(directory)
        relative_directory = directory_path.relative_to(output_root)
        observed_directories.add(
            "." if relative_directory == Path(".") else str(relative_directory)
        )
        if directory_path.is_symlink() or stat.S_IMODE(directory_path.stat().st_mode) != 0o700:
            raise ReceiptError("receipt directories must be non-symlink mode 0700")
        for name in directory_names:
            if (directory_path / name).is_symlink():
                raise ReceiptError("receipt contains a directory symlink")
        for name in file_names:
            path = directory_path / name
            relative = str(path.relative_to(output_root))
            if path.is_symlink():
                raise ReceiptError("receipt contains a file symlink")
            observed_files.add(relative)
    if observed_directories != {".", "events"}:
        raise ReceiptError("receipt directory set differs from the V1 corridor")
    if observed_files != expected_files:
        raise ReceiptError(
            f"receipt file set differs: missing={sorted(expected_files - observed_files)}, "
            f"unexpected={sorted(observed_files - expected_files)}"
        )
    for record in records:
        actual = _file_record(output_root / record["path"], record["path"], single_link=True)
        if actual != record:
            raise ReceiptError(f"artifact identity differs for {record['path']}")


def check(receipt_path: Path, require_release: bool = False) -> dict[str, Any]:
    receipt_path = _absolute(receipt_path)
    receipt_file, receipt_bytes = _read_file_record(
        receipt_path, "receipt.json", single_link=True
    )
    if receipt_file["mode"] != 0o600:
        raise ReceiptError("receipt.json mode must be 0600")
    try:
        receipt = json.loads(receipt_bytes.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ReceiptError("receipt is not UTF-8 JSON") from error
    if canonical_json(receipt) != receipt_bytes:
        raise ReceiptError("receipt JSON is not canonical")
    receipt = _require_keys(receipt, TOP_KEYS, "receipt")
    if receipt["schema"] != SCHEMA_NAME or receipt["schema_version"] != 1:
        raise ReceiptError("receipt schema differs")
    if receipt["mode"] != "formal-only":
        raise ReceiptError("receipt mode differs")
    if receipt["evidence_class"] != "diagnostic":
        raise ReceiptError("receipt evidence class differs")
    output_root = receipt_path.parent
    root = _absolute(Path(receipt["source_identity"]["root"]))
    if root.resolve(strict=True) != root:
        raise ReceiptError("source root is not canonical")
    invocation = _require_keys(
        receipt["invocation"],
        {"argv", "cwd", "environment", "timeout_seconds", "runtime_root"},
        "invocation",
    )
    if (
        not isinstance(invocation["argv"], list)
        or not all(isinstance(item, str) for item in invocation["argv"])
        or not isinstance(invocation["cwd"], str)
        or not isinstance(invocation["timeout_seconds"], (int, float))
        or not 1 <= invocation["timeout_seconds"] <= 86400
    ):
        raise ReceiptError("collector invocation values are invalid")
    environment = invocation["environment"]
    expected_environment = {
        "LANG": "C", "LC_ALL": "C", "TMPDIR": str(Path(invocation["runtime_root"]) / "tmp"), "TZ": "UTC"
    }
    if environment != expected_environment or Path(invocation["runtime_root"]).exists():
        raise ReceiptError("closed environment or runtime cleanup differs")

    source_identity = _require_keys(
        receipt["source_identity"], {"root", "files", "manifest_sha256"}, "source identity"
    )
    source_records = [_validate_record(item, "source file") for item in source_identity["files"]]
    expected_source_paths = set(BASE_SOURCE_PATHS) | _tla_closure(root)
    observed_source_paths = [item["path"] for item in source_records]
    if observed_source_paths != sorted(expected_source_paths):
        raise ReceiptError("source identity path set differs from the derived runner inputs")
    current_source_records = []
    for record in source_records:
        current = _file_record(root / record["path"], record["path"], single_link=True)
        if current != record:
            raise ReceiptError(f"source drift for {record['path']}")
        current_source_records.append(current)
    if source_identity["manifest_sha256"] != _manifest(current_source_records):
        raise ReceiptError("source manifest differs")

    runner = _require_keys(receipt["runner"], {"path", "sha256", "event_graph"}, "runner")
    collector = root / "scripts/formal/collect_sumeragi_v2_replay_receipt.py"
    collector_record = next(item for item in source_records if item["path"] == runner["path"])
    if runner["path"] != "scripts/formal/collect_sumeragi_v2_replay_receipt.py" or runner["sha256"] != collector_record["sha256"]:
        raise ReceiptError("runner identity differs")
    expected_graph = _source_event_graph(collector, receipt["mode"])
    if runner["event_graph"] != expected_graph:
        raise ReceiptError("event graph differs from the final collector source")

    if not isinstance(receipt["events"], list):
        raise ReceiptError("events are not a list")
    event_names = [event.get("name") if isinstance(event, dict) else None for event in receipt["events"]]
    if event_names != expected_graph["nodes"]:
        raise ReceiptError("event order differs from the source-derived graph")
    tools = _tool_locations(receipt, root)
    _validate_collector_invocation(invocation, receipt, root, output_root, tools)
    tool_identity = _require_keys(
        receipt["tool_identity"],
        {"tla2tools_version", "tlapm_commit", "files", "manifest_sha256"},
        "tool identity",
    )
    if tool_identity["tla2tools_version"] != "1.7.4" or tool_identity["tlapm_commit"] != "3ab43c7ff31db4ced850619d4746fa4c841a7681":
        raise ReceiptError("pinned formal tool versions differ")
    tool_records = [_validate_record(item, "tool file") for item in tool_identity["files"]]
    if [item["path"] for item in tool_records] != sorted(tools):
        raise ReceiptError("tool identity path set differs from event use")
    current_tool_records = []
    for record in tool_records:
        current = _file_record(tools[record["path"]], record["path"], single_link=record["path"].startswith("tlapm-") or record["path"] == "tool/tla2tools.jar")
        if current != record:
            raise ReceiptError(f"tool drift for {record['path']}")
        current_tool_records.append(current)
    hashes = {item["path"]: item["sha256"] for item in tool_records}
    if hashes.get("tool/tla2tools.jar") != TLA2TOOLS_SHA256:
        raise ReceiptError("TLA2Tools checksum differs")
    for logical, digest in TLAPM_HASHES.items():
        if hashes.get(logical) != digest:
            raise ReceiptError(f"{logical} checksum differs")
        if stat.S_IMODE(tools[logical].stat().st_mode) & 0o222:
            raise ReceiptError(f"{logical} is not sealed")
    projection = tools["tlapm-projection/Functions.tla"].parent
    if stat.S_IMODE(projection.stat().st_mode) & 0o222 or sorted(item.name for item in projection.iterdir()) != ["Folds.tla", "Functions.tla"]:
        raise ReceiptError("TLAPM projection is not the exact sealed two-file projection")
    if tool_identity["manifest_sha256"] != _manifest(current_tool_records):
        raise ReceiptError("tool manifest differs")

    for sequence, event in enumerate(receipt["events"], 1):
        _validate_event(event, sequence, root, output_root, tools, receipt)
    raw_path = output_root / next(
        event["outputs"]["stdout"]["path"] for event in receipt["events"] if event["name"] == "raw_tlc"
    )
    normalized_path = output_root / next(
        event["outputs"]["stdout"]["path"] for event in receipt["events"] if event["name"] == "normalizer"
    )
    sany_path = output_root / next(
        event["outputs"]["stdout"]["path"] for event in receipt["events"] if event["name"] == "standalone_sany"
    )
    raw_relative = str(raw_path.relative_to(output_root))
    normalized_relative = str(normalized_path.relative_to(output_root))
    sany_relative = str(sany_path.relative_to(output_root))
    raw_record, raw = _read_file_record(raw_path, raw_relative, single_link=True)
    normalized_record, normalized = _read_file_record(
        normalized_path, normalized_relative, single_link=True
    )
    sany_record, sany = _read_file_record(sany_path, sany_relative, single_link=True)
    event_output_records = {
        output["path"]: output
        for event in receipt["events"]
        for output in event["outputs"].values()
    }
    if (
        raw_record != event_output_records[raw_relative]
        or normalized_record != event_output_records[normalized_relative]
        or sany_record != event_output_records[sany_relative]
    ):
        raise ReceiptError("formal stdout changed after event validation")
    if CONTROL_RE.search(raw) or CONTROL_RE.search(sany) or DIAGNOSTIC_RE.search(sany):
        raise ReceiptError("formal stdout contains controls or hidden diagnostics")
    if sany.count(b"Semantic processing of module SumeragiV2TraceWitness") != 1:
        raise ReceiptError("standalone SANY did not complete the witness module exactly once")
    state_starts = len(re.findall(rb"^@!@!@STARTMSG 2217:4 @!@!@$", raw, re.MULTILINE))
    state_ends = len(re.findall(rb"^@!@!@ENDMSG 2217 @!@!@$", raw, re.MULTILINE))
    action_count = sum(1 for line in normalized.splitlines() if re.match(rb"^[0-9]+\t", line))
    fixture = root / "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv"
    fixture_record, fixture_bytes = _read_file_record(
        fixture,
        "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv",
        single_link=True,
    )
    source_record_by_path = {item["path"]: item for item in source_records}
    if fixture_record != source_record_by_path[fixture_record["path"]]:
        raise ReceiptError("normalized fixture changed after source validation")
    if (
        state_starts != EXPECTED_STATES
        or state_ends != EXPECTED_STATES
        or action_count != EXPECTED_ACTIONS
        or normalized != fixture_bytes
    ):
        raise ReceiptError("formal state/action counts or normalized bytes differ")
    result = _require_keys(
        receipt["result"],
        {
            "accepted", "sany_status", "tlc_status", "normalizer_status",
            "tool_states", "actions", "separate_stderr_empty", "normalized_fixture",
            "normalized_sha256", "normalized_matches_fixture",
        },
        "result",
    )
    expected_result = {
        "accepted": True,
        "sany_status": 0,
        "tlc_status": 12,
        "normalizer_status": 0,
        "tool_states": EXPECTED_STATES,
        "actions": EXPECTED_ACTIONS,
        "separate_stderr_empty": True,
        "normalized_fixture": "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv",
        "normalized_sha256": sha256_bytes(normalized),
        "normalized_matches_fixture": True,
    }
    if result != expected_result:
        raise ReceiptError("result contract differs")
    publication = _require_keys(
        receipt["publication"],
        {"create_only", "unexpected_files_allowed", "symlinks_allowed", "hard_links_allowed", "partial"},
        "publication",
    )
    if publication != {
        "create_only": True,
        "unexpected_files_allowed": False,
        "symlinks_allowed": False,
        "hard_links_allowed": False,
        "partial": False,
    }:
        raise ReceiptError("publication contract differs")
    _validate_signing(receipt, require_release)
    _validate_artifact_tree(receipt, output_root)
    return receipt


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--require-release", action="store_true")
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        receipt = check(args.receipt, args.require_release)
    except (OSError, UnicodeError, ValueError, ReceiptError) as error:
        print(f"Sumeragi V2 replay receipt verification failed: {error}", file=sys.stderr)
        return 2
    print(
        f"verified {receipt['result']['tool_states']} tool states and "
        f"{receipt['result']['actions']} replay actions ({receipt['evidence_class']})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
