#!/usr/bin/env python3
"""Measure one rustc child and reconcile its outputs with Cargo compile units.

This is private instrumentation for profile_cargo_build.py, not a second
profiling entry point. Each wait4 call owns exactly one compiler child.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import secrets
import shlex
import stat
import subprocess
import sys
import tempfile
import time
from typing import Any, Mapping, Sequence


RECORD_SCHEMA = 3
MAX_RECORD_BYTES = 16 * 1024 * 1024
MAX_RECORDS = 250_000
MAX_DIAGNOSTIC_BYTES = 16 * 1024 * 1024


def peak_rss_bytes(raw: int, platform_name: str) -> int:
    """Normalize this child's ru_maxrss, rejecting unsupported platforms."""
    if isinstance(raw, bool) or not isinstance(raw, int) or raw <= 0:
        raise ValueError("compiler peak RSS must be a positive integer")
    if platform_name == "darwin":
        return raw
    if platform_name.startswith("linux"):
        return raw * 1024
    raise ValueError(f"per-process RSS is unsupported on {platform_name}")


def option_values(arguments: Sequence[str], option: str) -> list[str]:
    """Read rustc's separate and equals spellings without shell parsing."""
    values = []
    for index, argument in enumerate(arguments):
        if argument == option:
            if index + 1 == len(arguments):
                raise ValueError(f"{option} requires a value")
            values.append(arguments[index + 1])
        elif argument.startswith(option + "="):
            values.append(argument[len(option) + 1:])
    return values


def compiler_profile(arguments: Sequence[str]) -> dict[str, Any]:
    """Resolve rustc codegen defaults for the profile fields Cargo reports."""
    options = {}
    index = 0
    while index < len(arguments):
        argument = arguments[index]
        if argument == "-C":
            index += 1
            if index == len(arguments):
                raise ValueError("-C requires a codegen option")
            value = arguments[index]
        elif argument.startswith("-C"):
            value = argument[2:]
        else:
            index += 1
            continue
        key, separator, value = value.partition("=")
        options[key] = value if separator else "yes"
        index += 1

    def boolean(key: str, default: bool) -> bool:
        if key not in options:
            return default
        value = options[key]
        if value in ("y", "yes", "on", "true"):
            return True
        if value in ("n", "no", "off", "false"):
            return False
        raise ValueError(f"invalid compiler {key} value")

    opt_level = options.get("opt-level", "0")
    debug_assertions = boolean("debug-assertions", opt_level == "0")
    debuginfo = options.get("debuginfo", "0")
    debuginfo = int(debuginfo) if debuginfo in ("0", "1", "2") else debuginfo
    return {
        "opt_level": opt_level, "debuginfo": debuginfo,
        "debug_assertions": debug_assertions,
        "overflow_checks": boolean("overflow-checks", debug_assertions),
        "test": "--test" in arguments,
    }


def invocation_identity(arguments: Sequence[str], cwd: Path) -> dict[str, Any]:
    """Separate compiler discovery probes from actual source compilation."""
    probe = any(
        value in ("-vV", "-V", "-Vv", "--version", "--print")
        or value.startswith("--print=")
        for value in arguments
    )
    if probe:
        return {"probe": True, "probe_source": None}
    names = option_values(arguments, "--crate-name")
    sources = [value for value in arguments if value.endswith(".rs")]
    # Build scripts (notably autocfg) compile real source through stdin without
    # Cargo's JSON artifact flags. They must see the real compiler's answer.
    if not sources and "-" in arguments:
        return {"probe": True, "probe_source": {"kind": "stdin", "path": None}}
    output_dir = os.environ.get("OUT_DIR")
    if len(sources) == 1:
        candidate = Path(sources[0])
        candidate = candidate if candidate.is_absolute() else cwd / candidate
        source = candidate.resolve(strict=True)
        probe_output = False
        if output_dir and Path(output_dir).is_absolute():
            owned_output = Path(output_dir).resolve(strict=True)
            outputs = option_values(arguments, "--out-dir") + option_values(arguments, "-o")
            probe_output = any(
                (Path(value) if Path(value).is_absolute() else cwd / value).resolve().is_relative_to(owned_output)
                for value in outputs
            )
        # Cargo always requests JSON diagnostics. Build scripts also compile
        # package-source files with ordinary diagnostics, not only OUT_DIR files.
        if option_values(arguments, "--error-format") != ["json"] or probe_output:
            return {"probe": True, "probe_source": {"kind": "file", "path": str(source)}}
    if len(names) != 1 or len(sources) != 1:
        raise ValueError("compile invocation needs one crate name and one Rust source")
    source = Path(sources[0])
    source = source if source.is_absolute() else cwd / source
    features = []
    for value in option_values(arguments, "--cfg"):
        if value.startswith("feature="):
            decoded = json.loads(value[len("feature="):])
            if not isinstance(decoded, str):
                raise ValueError("compiler feature must be a string")
            features.append(decoded)
    manifest = os.environ.get("CARGO_MANIFEST_DIR")
    if not manifest or not Path(manifest).is_absolute():
        raise ValueError("compile invocation lacks Cargo manifest identity")
    output_dir = os.environ.get("OUT_DIR")
    if output_dir is not None and not Path(output_dir).is_absolute():
        raise ValueError("compiler build-script output directory must be absolute")
    return {
        "probe": False,
        "crate_name": names[0],
        "source_path": str(source.resolve(strict=True)),
        "manifest_path": str((Path(manifest) / "Cargo.toml").resolve(strict=True)),
        "features": sorted(set(features)),
        "build_script_out_dir": str(Path(output_dir).resolve(strict=True)) if output_dir else None,
        "profile": compiler_profile(arguments),
        "crate_types": sorted(value for group in option_values(arguments, "--crate-type") for value in group.split(",")),
    }


def artifact_diagnostics(arguments: Sequence[str], probe: bool) -> list[str]:
    """Request stable rustc artifact messages while preserving diagnostics."""
    execution = list(arguments)
    if probe:
        return execution
    if option_values(arguments, "--error-format") != ["json"]:
        raise ValueError("measured compilation requires Cargo JSON diagnostics")
    options = option_values(arguments, "--json")
    if len(options) > 1:
        raise ValueError("compiler JSON options must be unique")
    if not options:
        execution.append("--json=artifacts")
    elif "artifacts" not in options[0].split(","):
        for index, argument in enumerate(execution):
            if argument == "--json":
                execution[index + 1] += ",artifacts"
                break
            if argument.startswith("--json="):
                execution[index] += ",artifacts"
                break
    return execution


def open_directory(path: Path) -> int:
    """Traverse an absolute directory using held descriptors, never symlinks."""
    if not path.is_absolute() or ".." in path.parts:
        raise ValueError("measurement directory must be absolute and canonical")
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0)
    descriptor = os.open(path.anchor, flags)
    try:
        for component in path.parts[1:]:
            child = os.open(component, flags, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def stable_file_identity(path: Path, *, maximum: int = 4 * 1024**3) -> dict[str, Any]:
    """Hash a regular artifact in bounded chunks and reject replacement/drift."""
    parent_fd = open_directory(path.parent)
    before = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)
    try:
        descriptor = os.open(path.name, flags, dir_fd=parent_fd)
    except BaseException:
        os.close(parent_fd)
        raise
    fields = ("st_dev", "st_ino", "st_mode", "st_size", "st_mtime_ns", "st_ctime_ns")
    fingerprint = lambda value: tuple(getattr(value, field) for field in fields)
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or fingerprint(before) != fingerprint(opened):
            raise ValueError(f"measurement input is not a stable regular file: {path}")
        if opened.st_size > maximum:
            raise ValueError(f"measurement input exceeds size limit: {path}")
        digest = hashlib.sha256()
        size = 0
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
            size += len(chunk)
            if size > maximum:
                raise ValueError(f"measurement input exceeds size limit: {path}")
        if (fingerprint(opened) != fingerprint(os.fstat(descriptor))
                or fingerprint(opened) != fingerprint(os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)) or size != opened.st_size):
            raise ValueError(f"measurement input changed while hashing: {path}")
        return {"bytes": size, "sha256": digest.hexdigest()}
    finally:
        os.close(descriptor)
        os.close(parent_fd)


def jobserver_descriptors(environment: Mapping[str, str]) -> tuple[int, ...]:
    """Preserve Cargo's inherited jobserver pipe without leaking other handles."""
    descriptors = None
    fifo = None
    for argument in shlex.split(environment.get("CARGO_MAKEFLAGS", "")):
        name, separator, value = argument.partition("=")
        if name not in ("--jobserver-auth", "--jobserver-fds"):
            continue
        if not separator or not value:
            raise ValueError("Cargo jobserver authentication is missing")
        if name == "--jobserver-auth" and value.startswith("fifo:"):
            if not value[5:] or descriptors is not None or (fifo is not None and fifo != value):
                raise ValueError("Cargo jobserver authentication is conflicting or malformed")
            # Named FIFO ownership remains with Cargo/rustc; no path is opened here.
            fifo = value
            continue
        values = value.split(",")
        if len(values) != 2 or any(not item.isascii() or not item.isdecimal() for item in values):
            raise ValueError("Cargo jobserver descriptor pair is malformed")
        pair = tuple(int(item) for item in values)
        if (any(item <= 2 for item in pair) or pair[0] == pair[1]
                or fifo is not None or (descriptors is not None and descriptors != pair)):
            raise ValueError("Cargo jobserver descriptor pair is conflicting or invalid")
        descriptors = pair
    if descriptors is None:
        return ()
    for descriptor in descriptors:
        try:
            os.fstat(descriptor)
        except OSError as error:
            raise ValueError("Cargo jobserver descriptor is not inherited") from error
    return descriptors


@contextmanager
def compiler_input(identity: dict[str, Any]):
    """Bind generated probe source bytes while preserving the compiler's stdin."""
    source = identity.get("probe_source")
    if source is None:
        yield None
    elif source["kind"] == "file":
        path = Path(source["path"])
        before = stable_file_identity(path)
        identity["probe_source"] = {**source, **before}
        yield None
        if stable_file_identity(path) != before:
            raise ValueError("compiler probe source changed while compiling")
    else:
        # rustc consumes the entire source before compilation. Spool with bounded
        # memory so we can hash stdin without stealing bytes from its child.
        with tempfile.TemporaryFile() as captured:
            digest = hashlib.sha256()
            size = 0
            while chunk := sys.stdin.buffer.read(1024 * 1024):
                captured.write(chunk)
                digest.update(chunk)
                size += len(chunk)
            captured.seek(0)
            identity["probe_source"] = {**source, "bytes": size, "sha256": digest.hexdigest()}
            yield captured


def measure_compiler(compiler: str, arguments: Sequence[str]) -> dict[str, Any]:
    """Forward compiler IO and wait for this process alone, including failures."""
    identity = invocation_identity(arguments, Path.cwd())
    execution = artifact_diagnostics(arguments, identity["probe"])
    with compiler_input(identity) as compiler_stdin:
        artifacts = []
        started = time.monotonic_ns()
        process = subprocess.Popen(
            [compiler, *execution], stdin=compiler_stdin, stderr=subprocess.PIPE,
            pass_fds=jobserver_descriptors(os.environ),
        )
        assert process.stderr is not None
        diagnostic_error = None
        with process.stderr:
            while line := process.stderr.readline(MAX_DIAGNOSTIC_BYTES + 1):
                # Cargo receives the compiler stream byte-for-byte. This helper does
                # not swallow diagnostics or let a pipe-reader error orphan rustc.
                try:
                    sys.stderr.buffer.write(line)
                    sys.stderr.buffer.flush()
                except OSError as error:
                    diagnostic_error = f"could not forward compiler diagnostics: {error}"
                if len(line) > MAX_DIAGNOSTIC_BYTES:
                    diagnostic_error = "compiler diagnostic exceeds size limit"
                    continue
                try:
                    message = json.loads(line)
                except (ValueError, UnicodeError):
                    continue
                if isinstance(message, dict) and "artifact" in message:
                    if not isinstance(message["artifact"], str):
                        diagnostic_error = "compiler artifact path must be a string"
                    else:
                        artifacts.append(message["artifact"])
        pid, status, usage = os.wait4(process.pid, 0)
        process.returncode = os.waitstatus_to_exitcode(status)
        elapsed_ns = time.monotonic_ns() - started
        if pid != process.pid:
            raise ValueError("wait4 returned a different compiler process")
        if diagnostic_error:
            raise ValueError(diagnostic_error)
        outputs = []
        for value in sorted(set(artifacts)):
            path = Path(value)
            if not path.is_absolute():
                path = Path.cwd() / path
            outputs.append({"path": str(path), **stable_file_identity(path)})
        return {
            "schema_version": RECORD_SCHEMA,
            **identity,
            "arguments": execution,
            "artifacts": outputs,
            "returncode": process.returncode,
            "elapsed_ns": elapsed_ns,
            "peak_rss_bytes": peak_rss_bytes(usage.ru_maxrss, sys.platform),
            "user_cpu_seconds": usage.ru_utime,
            "system_cpu_seconds": usage.ru_stime,
        }

def write_record(directory: Path, record: Mapping[str, Any]) -> None:
    """Publish a completed invocation into its private exclusive record file."""
    payload = json.dumps(record, sort_keys=True, separators=(",", ":")).encode() + b"\n"
    if len(payload) > MAX_RECORD_BYTES:
        raise ValueError("compiler measurement exceeds record limit")
    directory_fd = open_directory(directory)
    try:
        descriptor = os.open(
            secrets.token_hex(16) + ".json", os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            0o600, dir_fd=directory_fd,
        )
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
    finally:
        os.close(directory_fd)


def normalized_path(value: str, roots: Mapping[str, Path]) -> str:
    """Remove invocation-private root names from compiler/artifact evidence."""
    result = value
    for label, root in sorted(roots.items(), key=lambda item: -len(str(item[1]))):
        result = result.replace(str(root) + "/", label + "/")
        if result == str(root):
            result = label
    return result


def read_records(directory: Path) -> list[dict[str, Any]]:
    """Load bounded complete private records and reject ambiguous evidence."""
    directory_fd = open_directory(directory)
    try:
        entries = sorted(directory / name for name in os.listdir(directory_fd))
    finally:
        os.close(directory_fd)
    if len(entries) > MAX_RECORDS:
        raise ValueError("too many compiler measurement records")
    records = []

    def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        output = {}
        for key, value in pairs:
            if key in output:
                raise ValueError("duplicate compiler measurement key")
            output[key] = value
        return output

    for path in entries:
        before = stable_file_identity(path, maximum=MAX_RECORD_BYTES)
        if path.suffix != ".json":
            raise ValueError("unexpected compiler measurement entry")
        parent_fd = open_directory(path.parent)
        try:
            descriptor = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=parent_fd)
            with os.fdopen(descriptor, "rb") as source:
                payload = source.read(MAX_RECORD_BYTES + 1)
        finally:
            os.close(parent_fd)
        if before != {"bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}:
            raise ValueError("compiler measurement changed while read")
        record = json.loads(payload, object_pairs_hook=unique_object)
        if isinstance(record, dict) and "measurement_error" in record:
            raise ValueError(f'compiler instrumentation failed: {record["measurement_error"]}')
        common = {
            "schema_version", "probe", "arguments", "artifacts", "returncode",
            "elapsed_ns", "peak_rss_bytes", "user_cpu_seconds", "system_cpu_seconds",
        }
        if not isinstance(record, dict) or type(record.get("probe")) is not bool:
            raise ValueError("compiler measurement lacks invocation kind")
        keys = common | {"probe_source"} if record["probe"] else common | {
            "crate_name", "source_path", "manifest_path", "features", "profile", "crate_types", "build_script_out_dir",
        }
        if set(record) != keys or type(record["schema_version"]) is not int or record["schema_version"] != RECORD_SCHEMA:
            raise ValueError("compiler measurement schema mismatch")
        for key in ("peak_rss_bytes", "elapsed_ns"):
            if type(record[key]) is not int or record[key] <= 0:
                raise ValueError(f"compiler measurement {key} must be positive")
        if type(record["returncode"]) is not int:
            raise ValueError("compiler measurement returncode must be an integer")
        for key in ("user_cpu_seconds", "system_cpu_seconds"):
            value = record[key]
            if type(value) not in (int, float) or not 0 <= value < float("inf"):
                raise ValueError(f"compiler measurement {key} is invalid")
        if not isinstance(record["arguments"], list) or not all(isinstance(value, str) for value in record["arguments"]):
            raise ValueError("compiler measurement arguments are invalid")
        if not isinstance(record["artifacts"], list):
            raise ValueError("compiler measurement artifacts are invalid")
        if record["probe"]:
            source = record["probe_source"]
            if source is not None:
                if (not isinstance(source, dict) or set(source) != {"kind", "path", "bytes", "sha256"}
                        or source["kind"] not in ("stdin", "file")
                        or type(source["bytes"]) is not int or source["bytes"] < 0
                        or not isinstance(source["sha256"], str) or len(source["sha256"]) != 64
                        or any(c not in "0123456789abcdef" for c in source["sha256"])
                        or (source["kind"] == "stdin" and source["path"] is not None)
                        or (source["kind"] == "file" and (not isinstance(source["path"], str)
                            or not Path(source["path"]).is_absolute()))):
                    raise ValueError("compiler probe source identity is invalid")
        if not record["probe"]:
            output_dir = record["build_script_out_dir"]
            if output_dir is not None and (not isinstance(output_dir, str) or not Path(output_dir).is_absolute()):
                raise ValueError("compiler measurement build-script output directory is invalid")
            for key in ("crate_name", "source_path", "manifest_path"):
                if not isinstance(record[key], str) or not record[key]:
                    raise ValueError(f"compiler measurement {key} is invalid")
            for key in ("features", "crate_types"):
                if not isinstance(record[key], list) or not all(isinstance(value, str) for value in record[key]):
                    raise ValueError(f"compiler measurement {key} is invalid")
            if json.dumps(record["profile"], sort_keys=True) != json.dumps(compiler_profile(record["arguments"]), sort_keys=True):
                raise ValueError("compiler measurement profile disagrees with invocation")
        records.append(record)
    return records


def reconcile_measurements(
    messages: Sequence[dict[str, Any]], records: Sequence[dict[str, Any]],
    roots: Mapping[str, Path], project_unit: Any,
    *, build_script_messages: Sequence[dict[str, Any]] = (),
) -> dict[str, Any]:
    """Match each compiled Cargo artifact to exactly one measured invocation."""
    if not messages:
        raise ValueError("Cargo build emitted no compiler-artifact identity")
    target_root = roots["target"].resolve(strict=True)
    build_scripts = {}
    for message in build_script_messages:
        if (message.get("reason") != "build-script-executed"
                or not isinstance(message.get("package_id"), str)
                or not isinstance(message.get("out_dir"), str)
                or not isinstance(message.get("cfgs"), list)
                or not all(isinstance(value, str) for value in message["cfgs"])):
            raise ValueError("Cargo build-script identity is malformed")
        directory = Path(message["out_dir"])
        if not directory.is_absolute() or not directory.resolve(strict=True).is_relative_to(target_root):
            raise ValueError("Cargo build-script output directory escapes the measured target")
        features = set()
        for value in message["cfgs"]:
            if value.startswith("feature="):
                feature = json.loads(value[len("feature="):])
                if not isinstance(feature, str):
                    raise ValueError("Cargo build-script feature cfg is invalid")
                features.add(feature)
        key = (message["package_id"], str(directory.resolve(strict=True)))
        evidence = {"package_id": message["package_id"],
                    "out_dir": normalized_path(str(directory), roots), "cfgs": sorted(set(message["cfgs"]))}
        if key in build_scripts and build_scripts[key] != (features, evidence):
            raise ValueError("Cargo build-script evidence conflicts for one output directory")
        build_scripts[key] = (features, evidence)

    def checked_output(value: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(value, dict) or set(value) != {"path", "bytes", "sha256"}:
            raise ValueError("compiler output identity is malformed")
        if (not isinstance(value["path"], str) or type(value["bytes"]) is not int
                or value["bytes"] < 0 or not isinstance(value["sha256"], str)
                or len(value["sha256"]) != 64
                or any(character not in "0123456789abcdef" for character in value["sha256"])):
            raise ValueError("compiler output identity fields are invalid")
        path = Path(value["path"])
        if not path.is_absolute() or not path.resolve(strict=True).is_relative_to(target_root):
            raise ValueError("compiler output escapes the measured target")
        identity = stable_file_identity(path)
        if identity != {"bytes": value["bytes"], "sha256": value["sha256"]}:
            raise ValueError("compiler output identity changed after compilation")
        return {"path": normalized_path(str(path), roots), **identity}

    usable = []
    probes = []
    failed = []
    for record in records:
        metrics = {key: record[key] for key in (
            "elapsed_ns", "peak_rss_bytes", "user_cpu_seconds", "system_cpu_seconds", "returncode",
        )}
        metrics["arguments"] = [normalized_path(value, roots) for value in record["arguments"]]
        if record["probe"]:
            source = record["probe_source"]
            if source is None and record["artifacts"]:
                raise ValueError("compiler discovery probe unexpectedly emitted an artifact")
            metrics["source"] = ({**source, "path": normalized_path(source["path"], roots)
                                  if source["path"] is not None else None} if source else None)
            # Generated probes can delete their outputs immediately after rustc
            # returns. These are invocation evidence, not persistent Cargo units.
            metrics["compiler_artifacts"] = [
                {**artifact, "path": normalized_path(artifact["path"], roots)}
                for artifact in record["artifacts"]
            ]
            probes.append(metrics)
            continue
        if record["returncode"] != 0:
            failed.append({**metrics, "source_path": normalized_path(record["source_path"], roots)})
            continue
        outputs = [checked_output(value) for value in record["artifacts"]]
        if not outputs:
            raise ValueError("successful compiler invocation has no artifact measurements")
        usable.append((record, metrics, outputs))

    used = set()
    measurements = []
    fresh = []
    for message in messages:
        unit = project_unit(message)
        if unit is None:
            raise ValueError("Cargo compiler-artifact lacks complete unit identity")
        target = message.get("target", {})
        source = target.get("src_path")
        manifest = message.get("manifest_path")
        if not isinstance(source, str) or not isinstance(manifest, str):
            raise ValueError("Cargo compiler-artifact lacks source/manifest identity")
        filenames = message.get("filenames")
        if not isinstance(filenames, list) or not filenames or not all(isinstance(value, str) for value in filenames):
            raise ValueError("Cargo compiler-artifact lacks output identity")
        output_identities = []
        for filename in filenames:
            path = Path(filename)
            output_identities.append(checked_output({"path": filename, **stable_file_identity(path)}))
        if type(message.get("fresh")) is not bool:
            raise ValueError("Cargo compiler-artifact lacks freshness identity")
        if message["fresh"]:
            fresh.append({"unit": unit, "artifacts": output_identities})
            continue
        candidates = []
        candidate_scripts = {}
        for index, (record, metrics, outputs) in enumerate(usable):
            output_dir = record["build_script_out_dir"]
            script = None
            expected_features = set(unit["features"])
            if output_dir is not None:
                script_entry = build_scripts.get((message["package_id"], output_dir))
                if script_entry is None:
                    continue
                injected_features, script = script_entry
                script = {**script, "package_id": unit["package_id"]}
                expected_features.update(injected_features)
            if (record["crate_name"] != target["name"].replace("-", "_")
                    or record["source_path"] != str(Path(source).resolve(strict=True))
                    or record["manifest_path"] != str(Path(manifest).resolve(strict=True))
                    or record["features"] != sorted(expected_features)
                    or any(unit["profile"].get(key) != value for key, value in record["profile"].items())
                    or (record["crate_types"] and record["crate_types"] != sorted(target["crate_types"]))):
                continue
            emitted = {(value["bytes"], value["sha256"]) for value in outputs}
            if all((value["bytes"], value["sha256"]) in emitted for value in output_identities):
                candidates.append(index)
                candidate_scripts[index] = script
        if len(candidates) != 1 or candidates[0] in used:
            related = [
                {"crate_name": record["crate_name"], "features": record["features"],
                 "profile": record["profile"], "crate_types": record["crate_types"],
                 "source_path": normalized_path(record["source_path"], roots),
                 "manifest_path": normalized_path(record["manifest_path"], roots),
                 "artifacts": outputs}
                for record, _, outputs in usable
                if record["source_path"] == str(Path(source).resolve(strict=True))
            ]
            detail = {"unit": unit, "cargo_artifacts": output_identities,
                      "matching_invocations": len(candidates), "same_source_invocations": related}
            raise ValueError(
                "Cargo compiler-artifact has missing, duplicate, or mismatched RSS evidence: "
                + json.dumps(detail, sort_keys=True, separators=(",", ":"))
            )
        index = candidates[0]
        used.add(index)
        _, metrics, outputs = usable[index]
        measurements.append({
            "unit": unit, **metrics, "compiler_artifacts": outputs,
            "cargo_artifacts": output_identities,
            "build_script": candidate_scripts[index],
        })
    if len(used) != len(usable):
        raise ValueError("compiler measurement has no matching Cargo compiler-artifact")
    ordering = lambda value: json.dumps(value, sort_keys=True, separators=(",", ":"))
    measurements.sort(key=ordering)
    probes.sort(key=ordering)
    failed.sort(key=ordering)
    fresh.sort(key=ordering)
    return {"compiled": measurements, "fresh": fresh, "probes": probes, "failed": failed}


def main(argv: Sequence[str] | None = None) -> int:
    """Run only through the profiler's private pinned wrapper launcher."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record-dir", required=True, type=Path)
    parser.add_argument("--expected-compiler", required=True)
    parser.add_argument("compiler")
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    try:
        if args.compiler != args.expected_compiler:
            raise ValueError("Cargo selected a compiler outside the measured identity")
        if not hasattr(os, "wait4"):
            raise ValueError("per-process compiler measurements require wait4")
        record = measure_compiler(args.compiler, args.arguments)
        write_record(args.record_dir, record)
        code = record["returncode"]
        return code if code >= 0 else 128 - code
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        # Build scripts may interpret any compiler failure as an unsupported
        # feature. A wrapper failure must also invalidate the enclosing profile.
        try:
            write_record(args.record_dir, {
                "schema_version": RECORD_SCHEMA,
                "measurement_error": str(error),
                "arguments": args.arguments,
            })
        except (OSError, ValueError) as record_error:
            print(f"profile_rustc: could not record instrumentation failure: {record_error}", file=sys.stderr)
        print(f"profile_rustc: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
