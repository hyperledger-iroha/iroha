#!/usr/bin/env python3
"""Capture real Norito identities in an isolated dirty-source snapshot.

Requires Python 3.11+, Git, the workspace Rust toolchain and cached Cargo
dependencies. `prepare` copies Git-selected inputs using the authoritative
build profiler's source sealer, then instruments only the copied derives.
`run` executes one explicit Cargo test target, retaining its log, harness
identity, source seals and compiler-resolved records. No environment variable
is required; inherited build environment is fingerprinted, never printed.
This is migration evidence, not wire-fixture or release qualification.
Existing destinations are never replaced. Shipping source is never edited.
"""

from __future__ import annotations

import argparse
from dataclasses import asdict
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
from typing import Any


_SPEC = importlib.util.spec_from_file_location(
    "iroha_capture_profiler", Path(__file__).with_name("profile_cargo_build.py")
)
assert _SPEC and _SPEC.loader
PROFILER = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = PROFILER
_SPEC.loader.exec_module(PROFILER)
DERIVE = "crates/norito_derive/src/lib.rs"
PROBE = "scripts/norito_schema_capture/derive_probe.rs"
FILTER = "__norito_schema_capture_"
MARKER = "NORITO_SCHEMA_CAPTURE_V1\t"
SKIP = "NORITO_SCHEMA_CAPTURE_SKIP_V1\t"


def require(condition: bool, message: str) -> None:
    """Reject evidence that cannot be interpreted unambiguously."""
    if not condition:
        raise ValueError(message)


def digest(value: Any) -> str:
    """Hash structured inputs with the authoritative canonical encoding."""
    return PROFILER.sha256_bytes(PROFILER.canonical_json_bytes(value))


def write_json(path: Path, value: Any) -> None:
    """Create an evidence file without overwriting an earlier result."""
    with path.open("x", encoding="utf-8") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def instrument(source: str) -> str:
    """Append capture tests while keeping original codec implementation bodies."""
    require("mod schema_capture;" not in source, "derive source is already instrumented")
    anchor = "mod schema_identity;\n"
    require(source.count(anchor) == 1, "schema module anchor is ambiguous")
    result = source.replace(anchor, anchor + (
        '\n#[path = "../../../scripts/norito_schema_capture/derive_probe.rs"]\n'
        "mod schema_capture;\n"
    ))
    for suffix, direction in (("serialize", "Serialize"), ("deserialize", "Deserialize")):
        signature = f"pub fn derive_norito_{suffix}(input: TokenStream) -> TokenStream {{"
        require(result.count(signature) == 1, f"{suffix} entry point is ambiguous")
        wrapper = f"""{signature}
    let capture_input = input.clone();
    let mut output = derive_norito_{suffix}_without_capture(input);
    if let Ok(parsed) = syn::parse::<DeriveInput>(capture_input) {{
        if let Ok(attrs) = ContainerAttr::parse(&parsed.attrs) {{
            output.extend(TokenStream::from(schema_capture::probe(
                &parsed, schema_capture::Direction::{direction}, attrs.schema_name.as_deref(),
            )));
        }}
    }}
    output
}}

fn derive_norito_{suffix}_without_capture(input: TokenStream) -> TokenStream {{"""
        result = result.replace(signature, wrapper)
    return result


def tree_seal(root: Path) -> dict[str, Any]:
    """Seal all snapshot inputs, including unexpected files written by a build."""
    return asdict(PROFILER.bounded_tree_fingerprint(
        root, reject_hardlinks=True, allow_missing_symlink_targets=True,
    ))


def selected_sources(root: Path, environment: dict[str, str], git: str) -> dict[str, Any]:
    """Expand initialized Git links into explicit, independently sealed file paths.

    The build profiler intentionally rejects directory entries. Capture uses its
    existing file sealer with a flattened inventory; no linked checkout, dirty
    file or absent Git link is silently omitted from migration provenance.
    """
    paths, links = [], []

    def visit(checkout: Path, prefix: str, depth: int) -> None:
        require(depth < 16, "Git-link nesting exceeds capture limit")
        PROFILER.validate_git_worktree(checkout, environment, git, root.parent)
        entries = subprocess.check_output(
            [*PROFILER._closed_git_command(git, checkout), "ls-files", "--stage", "-z"],
            env=environment, cwd=root.parent,
        )
        gitlinks = {}
        for entry in entries.split(b"\0"):
            if not entry:
                continue
            metadata, separator, raw_path = entry.partition(b"\t")
            require(bool(separator), "invalid Git index record")
            mode, revision, stage = metadata.decode("ascii").split()
            require(stage == "0", "resolve Git index conflicts before capturing identities")
            if mode == "160000":
                gitlinks[raw_path.decode("utf-8", "surrogateescape")] = revision
        selected = PROFILER.tracked_and_untracked_paths(checkout, environment, git, root.parent)
        for relative in selected:
            physical = checkout / relative
            full_path = prefix + relative
            if relative in gitlinks:
                require(not physical.is_symlink(), "Git-link checkout cannot be a symlink")
                initialized = physical.is_dir()
                revision = None
                if initialized:
                    revision = subprocess.check_output(
                        [*PROFILER._closed_git_command(git, physical), "rev-parse", "HEAD"],
                        env=environment, cwd=root.parent, text=True,
                    ).strip()
                    require(re.fullmatch(r"[0-9a-f]{40,64}", revision) is not None,
                            "invalid Git-link checkout revision")
                links.append({"path": full_path, "indexed_revision": gitlinks[relative],
                              "checkout_revision": revision, "initialized": initialized})
                if initialized:
                    visit(physical, full_path + "/", depth + 1)
                else:
                    paths.append(full_path)
            else:
                require(not physical.is_dir() or physical.is_symlink(),
                        f"Git-selected directory is not a declared Git link: {full_path}")
                paths.append(full_path)

    visit(root, "", 0)
    require(len(paths) == len(set(paths)), "duplicate source path across Git worktrees")
    return {"paths": sorted(paths), "git_links": sorted(links, key=lambda item: item["path"])}


def prepare(root: Path, destination: Path) -> dict[str, Any]:
    """Copy a coherent source tree and instrument only its derive entry points."""
    root = root.resolve(strict=True)
    destination = destination.parent.resolve(strict=True) / destination.name
    require(not destination.exists(), "capture destination already exists")
    require(not destination.is_relative_to(root), "capture destination must be outside the repository")
    git = shutil.which("git")
    require(git is not None, "Git is required to select source inputs")
    environment = dict(os.environ)
    selection = selected_sources(root, environment, git)
    paths = selection["paths"]
    require(DERIVE in paths and PROBE in paths, "capture sources are not in the Git-selected input")
    original = (root / DERIVE).read_text(encoding="utf-8")
    # Validate the exact transformation before allocating a snapshot.
    patched = instrument(original)
    destination.mkdir(mode=0o700)
    snapshot = destination / "source"
    baseline = PROFILER.capture_source_snapshot(root, paths, snapshot)
    require((snapshot / DERIVE).read_text(encoding="utf-8") == original,
            "derive source changed before snapshot capture; prepare a new destination")
    (snapshot / DERIVE).write_text(patched, encoding="utf-8")
    require(selection == selected_sources(root, environment, git),
            "Git-selected source inventory changed during preparation")
    require(PROFILER.source_fingerprint(root, paths) == baseline,
            "working source changed during preparation")
    manifest = {
        "schema_version": 1,
        "purpose": "compiler identity capture; not release qualification",
        "original_root": str(root),
        "git_revision": subprocess.check_output(
            [git, "-C", str(root), "rev-parse", "HEAD"], text=True, env=environment,
        ).strip(),
        "source": asdict(baseline),
        "source_paths": paths,
        "source_paths_sha256": digest(paths),
        "git_links": selection["git_links"],
        "execution_source": tree_seal(snapshot),
        "instrumentation": {
            "path": DERIVE,
            "before_sha256": PROFILER.sha256_bytes(original.encode()),
            "after_sha256": PROFILER.sha256_bytes(patched.encode()),
            "probe_sha256": PROFILER.sha256_bytes((snapshot / PROBE).read_bytes()),
            "driver_sha256": PROFILER.sha256_bytes(Path(__file__).read_bytes()),
        },
    }
    write_json(destination / "capture.json", manifest)
    return manifest


def unhex(value: str) -> str:
    """Decode canonical hex-escaped UTF-8 capture fields."""
    require(bool(re.fullmatch(r"(?:[0-9a-f]{2})+", value)), "invalid capture hex field")
    return bytes.fromhex(value).decode("utf-8")


def parse_record(line: str) -> dict[str, Any] | None:
    """Read one probe line, accepting libtest's same-line test-name prefix."""
    markers = [marker for marker in (MARKER, SKIP) if marker in line]
    if not markers:
        return None
    require(len(markers) == 1 and line.count(markers[0]) == 1, "ambiguous capture line")
    marker = markers[0]
    prefix, payload = line.split(marker, 1)
    require(not prefix or bool(re.fullmatch(r"test [^\r\n]+ \.\.\. ", prefix)),
            "capture marker appears outside probe output")
    values = payload.rstrip("\r\n").split("\t")
    captured = marker == MARKER
    require(len(values) == (11 if captured else 7), "wrong capture field count")
    require(values[0] in ("serialize", "deserialize"), "invalid codec direction")
    if captured:
        direction, nominal, root, schema_hash, file, line_no, column, ident, module, explicit, matches = values
        require(bool(re.fullmatch(r"[0-9a-f]{32}", schema_hash)), "invalid schema hash")
        require(explicit in ("true", "false") and matches in ("true", "false"), "invalid capture boolean")
        record = {
            "kind": "capture", "nominal": unhex(nominal), "root_hint": unhex(root),
            "schema_hash": schema_hash, "explicit_root": explicit == "true",
            "root_matches": matches == "true",
        }
    else:
        direction, ident, file, line_no, column, module, reason = values
        require(reason == "generic", "unknown capture skip reason")
        record = {"kind": "review", "reason": reason}
    require(bool(re.fullmatch(r"[1-9][0-9]*", line_no)) and
            bool(re.fullmatch(r"[1-9][0-9]*", column)), "invalid source position")
    record.update(direction=direction, file=unhex(file), line=int(line_no), column=int(column),
                  identifier=unhex(ident), module=unhex(module))
    return record


def collect(log: Path) -> dict[str, Any]:
    """Retain compiler artifacts and records without treating test success as coverage."""
    records, artifacts, summaries = [], [], []
    identities: dict[tuple[Any, ...], dict[str, Any]] = {}
    build_success = False
    with log.open(encoding="utf-8") as stream:
        for line in stream:
            if line.startswith("{"):
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if event.get("reason") == "build-finished":
                    build_success = event.get("success") is True
                elif event.get("reason") == "compiler-artifact" and event.get("executable"):
                    artifacts.append(event)
                # Diagnostics can quote a marker; they are never runtime records.
                continue
            record = parse_record(line)
            if record is not None:
                key = tuple(record[field] for field in (
                    "direction", "file", "line", "column", "identifier", "module",
                ))
                require(key not in identities, "duplicate capture site in one harness run")
                identities[key] = record
                records.append(record)
            match = re.fullmatch(
                r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; .*\n?", line,
            )
            if match:
                summaries.append({"outcome": match[1], "passed": int(match[2]),
                                  "failed": int(match[3]), "ignored": int(match[4])})
    require(len(summaries) == 1, "expected exactly one selected test harness summary")
    require(len(artifacts) == 1, "expected exactly one Cargo test executable")
    summary = summaries[0]
    require(bool(records), "selected harness emitted no capture probes")
    require(summary["passed"] == len(records), "probe count differs from passed tests")
    return {"build_success": build_success, "summary": summary, "records": records,
            "compiler_artifact": artifacts[0], "coverage_complete": False}


def compiled_harness(log: Path) -> dict[str, Any]:
    """Select one newly compiled test artifact before invoking it directly."""
    artifacts, completions = [], []
    with log.open(encoding="utf-8") as stream:
        for line in stream:
            if not line.startswith("{"):
                continue
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("reason") == "compiler-artifact" and event.get("executable"):
                artifacts.append(event)
            elif event.get("reason") == "build-finished":
                completions.append(event.get("success"))
    require(completions == [True], "Cargo did not complete exactly one successful build")
    require(len(artifacts) == 1, "expected exactly one compiled test artifact")
    artifact = artifacts[0]
    require(artifact.get("fresh") is False and artifact.get("profile", {}).get("test") is True,
            "capture harness must be newly compiled as a test")
    return artifact


def tool_path(name: str, explicit: str | None, cwd: Path) -> Path:
    """Resolve the toolchain binary rather than fingerprinting a rustup proxy."""
    if explicit:
        return Path(explicit).resolve(strict=True)
    rustup = shutil.which("rustup")
    if rustup:
        result = subprocess.run([rustup, "which", name], cwd=cwd, capture_output=True, text=True, check=True)
        return Path(result.stdout.strip()).resolve(strict=True)
    resolved = shutil.which(name)
    require(resolved is not None, f"{name} is unavailable")
    return Path(resolved).resolve(strict=True)


def run_capture(args: argparse.Namespace) -> dict[str, Any]:
    """Run a selected target and bind its evidence to stable source and tools."""
    destination = args.snapshot.resolve(strict=True)
    source = destination / "source"
    manifest_bytes = (destination / "capture.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    require(manifest["schema_version"] == 1, "unknown capture manifest version")
    require(tree_seal(source) == manifest["execution_source"], "capture source changed after preparation")
    require(re.fullmatch(r"[A-Za-z0-9_-]+", args.name) is not None, "invalid run name")
    output = destination / "runs" / args.name
    output.mkdir(parents=True, mode=0o700)
    cargo = tool_path("cargo", args.cargo, source)
    rustc = tool_path("rustc", args.rustc, source)
    command = [str(cargo), "test", "--no-run", "--locked", "--offline", "-j", str(args.jobs),
               "--message-format=json-render-diagnostics", "-p", args.package]
    command += ["--test", args.test] if args.test else ["--lib"]
    if args.no_default_features:
        command.append("--no-default-features")
    if args.features:
        command += ["--features", args.features]
    target = output / "target"
    environment = dict(os.environ)
    # Empty values override Cargo's config wrappers as well as inherited ones.
    # Each create-only run owns a fresh target; no wrapped harness is reused.
    environment.update(CARGO_TARGET_DIR=str(target), CARGO_INCREMENTAL="0",
                       RUSTC=str(rustc), RUSTC_WRAPPER="", RUSTC_WORKSPACE_WRAPPER="",
                       VERGEN_GIT_SHA=manifest["git_revision"])
    tools = {}
    for name, path in (("cargo", cargo), ("rustc", rustc)):
        tools[name] = {"path": str(path), "identity": PROFILER.RUSTC_PROFILE.stable_file_identity(path),
                       "version": subprocess.check_output([str(path), "-vV"], cwd=source,
                                                          env=environment, text=True)}
    request = {"schema_version": 1, "command": command, "tools": tools,
               "environment_sha256": digest(environment),
               "capture_manifest_sha256": PROFILER.sha256_bytes(manifest_bytes),
               "execution_source": manifest["execution_source"],
               "compiler_controls": {"rustc_wrapper": "", "rustc_workspace_wrapper": "",
                                     "fresh_target": str(target), "direct_harness_execution": True},
               "driver_sha256": PROFILER.sha256_bytes(Path(__file__).read_bytes())}
    write_json(output / "request.json", request)
    started = time.monotonic_ns()
    log = output / "cargo.log"
    with log.open("x", encoding="utf-8") as stream:
        returncode = subprocess.run(command, cwd=source, env=environment, stdout=stream,
                                    stderr=subprocess.STDOUT, check=False).returncode
    result: dict[str, Any] = {"schema_version": 1, "returncode": returncode,
                             "build_returncode": returncode,
                             "request_sha256": digest(request), "valid": False}
    try:
        require(returncode == 0, "Cargo capture build failed")
        artifact = compiled_harness(log)
        executable = Path(artifact["executable"]).resolve(strict=True)
        require(executable.is_relative_to(target), "harness is outside capture target")
        result["harness"] = PROFILER.RUSTC_PROFILE.stable_file_identity(executable)
        result["harness_command"] = [str(executable), FILTER, "--nocapture", "--test-threads=1"]
        require(tree_seal(source) == manifest["execution_source"], "build mutated capture source")
        with log.open("a", encoding="utf-8") as stream:
            result["harness_returncode"] = subprocess.run(
                result["harness_command"], cwd=source, env=environment, stdout=stream,
                stderr=subprocess.STDOUT, check=False,
            ).returncode
        result["returncode"] = result["harness_returncode"]
        result["source_after"] = tree_seal(source)
        require(result["source_after"] == manifest["execution_source"], "build mutated capture source")
        for tool in tools.values():
            require(PROFILER.RUSTC_PROFILE.stable_file_identity(Path(tool["path"])) == tool["identity"],
                    "compiler tool changed during capture")
        result.update(collect(log))
        require(result["returncode"] == 0 and result["build_success"], "capture harness failed")
        require(result["summary"]["outcome"] == "ok" and result["summary"]["ignored"] == 0,
                "capture tests did not all execute successfully")
        require(result["compiler_artifact"] == artifact, "Cargo artifact changed in capture log")
        require(PROFILER.RUSTC_PROFILE.stable_file_identity(executable) == result["harness"],
                "harness changed during execution")
        require(PROFILER.sha256_bytes(Path(__file__).read_bytes()) == request["driver_sha256"],
                "capture driver changed during execution")
        require((destination / "capture.json").read_bytes() == manifest_bytes,
                "capture manifest changed during execution")
        result["valid"] = True
    except (ValueError, OSError, UnicodeError) as error:
        result["error"] = str(error)
    result["elapsed_ns"] = time.monotonic_ns() - started
    result["log"] = PROFILER.RUSTC_PROFILE.stable_file_identity(log)
    write_json(output / "result.json", result)
    return result


def main() -> int:
    """Expose separate snapshot preparation and explicit target capture actions."""
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)
    preparation = subparsers.add_parser("prepare", help="copy and instrument source outside the repository")
    preparation.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    preparation.add_argument("--out", required=True, type=Path)
    run = subparsers.add_parser("run", help="capture one selected library or integration-test harness")
    run.add_argument("--snapshot", required=True, type=Path)
    run.add_argument("--name", required=True, help="unique run name; previous evidence is never replaced")
    run.add_argument("--package", required=True)
    run.add_argument("--test", help="named integration test target; otherwise use the library")
    run.add_argument("--features")
    run.add_argument("--no-default-features", action="store_true")
    run.add_argument("--jobs", type=int, default=2)
    run.add_argument("--cargo", help="absolute toolchain Cargo binary; defaults to rustup which cargo")
    run.add_argument("--rustc", help="absolute toolchain rustc binary; defaults to rustup which rustc")
    args = parser.parse_args()
    try:
        if args.action == "prepare":
            result = prepare(args.root, args.out)
            print(json.dumps({"snapshot": str(args.out), "source": result["source"],
                              "execution_source": result["execution_source"]}, sort_keys=True))
            return 0
        require(args.jobs > 0, "jobs must be positive")
        result = run_capture(args)
        print(json.dumps({key: result[key] for key in ("valid", "returncode", "error") if key in result},
                         sort_keys=True))
        return 0 if result["valid"] else 1
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"norito capture: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
