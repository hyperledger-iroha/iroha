"""Persistent-checkout diagnostics for profile_cargo_build.py --local.

Requires Unix, Python 3.10+, the installed pinned Cargo toolchain and an offline
populated cache. This is a helper, not a separate profiling entry point. It runs
cargo_fast.sh in the caller checkout, never cleans its persistent target, and
emits local diagnostic evidence distinct from the isolated release report.
"""

from __future__ import annotations

import argparse
from contextlib import ExitStack
from dataclasses import asdict
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import stat
import statistics
import subprocess
import sys
import time

TIMING_PREFIX = "[cargo-fast-timings] "


def cargo_arguments(arguments, jobs):
    """Keep the caller's local profile while requiring offline JSON evidence."""
    result = list(arguments[1:] if arguments[:1] == ["--"] else arguments)
    if not result or result[0] not in ("check", "build", "test"):
        raise ValueError("local profiles require check, build or compile-only test")
    forbidden = {"--target-dir", "--build-dir", "--out-dir", "--artifact-dir", "--config",
                 "--message-format", "--timings", "--help", "-h", "--version", "-V", "--list"}
    if any(value.split("=", 1)[0] in forbidden for value in result) or "--" in result:
        raise ValueError("local profiler owns output controls; test execution is unsupported")
    for option in ("--locked", "--offline", "--message-format=json", "--timings"):
        if option not in result:
            result.append(option)
    if result[0] == "test" and "--no-run" not in result:
        result.append("--no-run")
    if jobs is not None:
        if any(value in ("-j", "--jobs") or value.startswith(("-j", "--jobs=")) for value in result):
            raise ValueError("select jobs once, before the Cargo separator")
        result += ["--jobs", str(jobs)]
    return result


def phase_times(stderr):
    """Require exactly one Cargo timing record."""
    records = [json.loads(line[len(TIMING_PREFIX):]) for line in stderr.splitlines()
               if line.startswith(TIMING_PREFIX)]
    if len(records) != 1 or not isinstance(records[0], dict):
        raise ValueError("missing or duplicate local phase timing record")
    result = records[0]
    if result.keys() != {"cargo_seconds"} or any(
        type(value) not in (int, float) or not 0 <= value < float("inf")
        for value in result.values()
    ):
        raise ValueError("invalid local phase timing record")
    return result


def target_size(root):
    """Count allocated and logical cache bytes without following target links."""
    result = {"files": 0, "bytes": 0, "allocated_bytes": 0}
    for parent, _, files in os.walk(root, followlinks=False):
        for name in files:
            info = (Path(parent) / name).lstat()
            if stat.S_ISREG(info.st_mode):
                result["files"] += 1
                result["bytes"] += info.st_size
                result["allocated_bytes"] += info.st_blocks * 512
    return result


def private_directory(path):
    """Create or reuse a caller-owned private directory without replacing it."""
    try:
        path.mkdir(mode=0o700)
    except FileExistsError:
        pass
    info = path.lstat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid()
            or stat.S_IMODE(info.st_mode) != 0o700 or path.resolve() != path):
        raise ValueError(f"unsafe local profiler state directory: {path}")
    return path


def pinned_file(path, payload, mode):
    """Keep a stable instrument path; refuse foreign or modified prior content."""
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    except FileExistsError:
        info = path.lstat()
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
                or info.st_nlink != 1 or stat.S_IMODE(info.st_mode) != mode
                or path.read_bytes() != payload):
            raise ValueError(f"local profiler instrument changed: {path}")
    else:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)


def compiler_launcher(state, compiler, api, environment):
    """Use the maintained per-child RSS helper at a persistent hashed path."""
    helper = api._HELPER_PATH.read_bytes()
    python = str(Path(sys.executable).resolve(strict=True))
    identity = {"compiler": str(compiler), "python": python,
                "compiler_bytes": api.RUSTC_PROFILE.stable_file_identity(compiler),
                "python_bytes": api.RUSTC_PROFILE.stable_file_identity(Path(python)),
                "helper_sha256": hashlib.sha256(helper).hexdigest()}
    key = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    directory = private_directory(state / key)
    records = private_directory(directory / "records")
    pinned_file(directory / "profile_rustc.py", helper, 0o400)
    command = [python, "-I", "-S", str(directory / "profile_rustc.py"),
               "--record-dir", str(records), "--expected-compiler", str(compiler), str(compiler)]
    pinned_file(directory / "rustc", ("#!/bin/sh\nexec " + shlex.join(command) + ' "$@"\n').encode(), 0o500)
    environment.update(RUSTC=str(directory / "rustc"), RUSTC_WRAPPER="", RUSTC_WORKSPACE_WRAPPER="")
    environment["PATH"] = str(directory) + os.pathsep + environment.get("PATH", "")
    return records, identity


def host_load():
    """Record load and compiler process identities without command-line secrets."""
    processes = subprocess.check_output(
        ["ps", "-axo", "pid=,ppid=,pcpu=,comm="], text=True,
    )
    return {"load_average": os.getloadavg(), "compiler_processes": [
        line.strip() for line in processes.splitlines()
        if Path(line.split(maxsplit=3)[-1]).name in ("cargo", "rustc", "sccache")
    ]}


def summarize(samples):
    """Summarize measured samples only, preserving failures and rebuilt counts."""
    if not samples:
        return {}
    keys = ("elapsed_seconds", "cargo_seconds")
    result = {key: {"median": statistics.median(sample[key] for sample in samples),
                    "worst": max(sample[key] for sample in samples)} for key in keys}
    result["all_successful"] = all(sample["returncode"] == 0 for sample in samples)
    result["zero_rebuilt_artifacts"] = all(sample["compiled"] == 0 for sample in samples)
    return result


def cargo_evidence(lines):
    """Require a completed build with actual artifact evidence, not Cargo help."""
    messages = []
    for line in lines:
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            messages.append(value)
    finishes = [value for value in messages if value.get("reason") == "build-finished"]
    if (len(finishes) != 1 or finishes[0].get("success") is not True
            or not any(value.get("reason") == "compiler-artifact" for value in messages)):
        raise ValueError("Cargo did not report one successful build with artifact evidence")
    return messages


def git_environment(environment):
    """Close only Git discovery; preserve the caller's separate Cargo environment."""
    result = {key: value for key, value in environment.items() if not key.startswith("GIT_")}
    result.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_SYSTEM=os.devnull,
                  GIT_CONFIG_GLOBAL=os.devnull, GIT_ATTR_NOSYSTEM="1",
                  GIT_OPTIONAL_LOCKS="0", GIT_NO_LAZY_FETCH="1",
                  GIT_TERMINAL_PROMPT="0", GIT_PAGER="", LANG="C", LC_ALL="C")
    return result


def source_snapshot(root, environment, git, api):
    """Fingerprint stable local membership, including only uninitialized gitlinks.

    The optional sibling documentation checkout may be absent or an empty
    directory. Its indexed commit and state remain part of the source identity;
    populated submodules are never silently omitted from these diagnostics.
    """
    environment = git_environment(environment)
    identity = api._directory_identity(root)
    api.validate_git_worktree(root, environment, git, root)
    index = api.git_index_entries(root, environment, git, root)
    paths = api.tracked_and_untracked_paths(root, environment, git, root)
    gitlink_paths = {path for path, mode, _oid, _stage in index if mode == "160000"}
    gitlinks = {}
    for path in gitlink_paths:
        entries = [entry for entry in index if entry[0] == path]
        if len(entries) != 1 or entries[0][3] != 0:
            raise ValueError(f"conflicted local gitlink is unsupported: {path}")
        gitlinks[path] = entries[0][2]
    # Keep the filesystem witnesses alive through the final Git membership reads,
    # so swapping one empty directory for another cannot disappear into its digest.
    with ExitStack() as checks:
        descriptor = api._open_directory_anchored(root)
        checks.callback(os.close, descriptor)
        if api._identity(os.fstat(descriptor)) != identity:
            raise ValueError("local source root changed during inventory")
        for path, oid in sorted(gitlinks.items()):
            checks.enter_context(api._uninitialized_gitlink_record(descriptor, path, oid))
        fingerprint = api.source_fingerprint(root, paths, expected_identity=identity, gitlinks=gitlinks)
        if (paths != api.tracked_and_untracked_paths(root, environment, git, root)
                or index != api.git_index_entries(root, environment, git, root)):
            raise ValueError("local source membership or Git index changed during snapshot")
        api.validate_git_worktree(root, environment, git, root)
        if not api._path_still_names(root, identity):
            raise ValueError("local source root changed during snapshot")
        return fingerprint


def run(argv, api):
    """Measure a stable local checkout repeatedly, keeping evidence on refusal."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--warmup", type=int, default=0)
    parser.add_argument("--jobs", type=int)
    parser.add_argument("--measure-compiler", action="store_true")
    parser.add_argument("--label", default="local-build")
    parser.add_argument("cargo_args", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    if args.repeat < 1 or args.warmup < 0 or (args.jobs is not None and args.jobs < 1):
        parser.error("repeat/jobs must be positive; warmup must be nonnegative")
    report = {"schema": "iroha-local-build-diagnostic-v1", "label": args.label,
              "release_qualification": False, "samples": [], "warmup": []}
    try:
        root, target, out = args.root.resolve(strict=True), args.target_dir.resolve(), args.out.resolve()
        if out.is_relative_to(root) or out.is_relative_to(target):
            raise ValueError("local profiling reports must be outside the checkout and target")
        arguments = cargo_arguments(args.cargo_args, args.jobs)
        out.parent.mkdir(parents=True, exist_ok=True)
        # Exclusive report ownership is established before any build or cache write.
        with out.open("x", encoding="utf-8") as output:
            try:
                environment = os.environ.copy()
                environment["CARGO_FAST_TIMINGS"] = "1"
                environment["CARGO_TARGET_DIR"] = str(target)
                if any(environment.get(key) for key in ("RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER")):
                    raise ValueError("unset custom compiler/wrapper overrides for a pinned local profile")
                compiler = Path(subprocess.check_output(["rustup", "which", "rustc"], cwd=root, text=True).strip()).resolve(strict=True)
                cargo = Path(subprocess.check_output(["rustup", "which", "cargo"], cwd=root, text=True).strip()).resolve(strict=True)
                environment["RUSTC"] = str(compiler)
                environment["RUSTC_WRAPPER"] = ""
                environment["RUSTC_WORKSPACE_WRAPPER"] = ""
                environment["PATH"] = str(cargo.parent) + os.pathsep + environment.get("PATH", "")
                # The existing read-only guard owns target/release-lane selection.
                # It must run before this profiler creates even its private lock.
                subprocess.run([sys.executable, str(root / "scripts/check_cargo_target_owner.py"),
                                "--source-root", str(root), "--", *arguments],
                               cwd=root, env=environment, check=True)
                target.mkdir(parents=True, exist_ok=True)
                state = private_directory(target / ".iroha-local-profile")
                lock_path = state / "lock"
                pinned_file(lock_path, b"", 0o600)
                with lock_path.open("r+") as lock:
                    fcntl.flock(lock, fcntl.LOCK_EX)
                    git = shutil.which("git")
                    if git is None:
                        raise ValueError("Git is required for source inventory")
                    def snapshot():
                        return asdict(source_snapshot(root, environment, git, api))
                    report.update(root=str(root), target=str(target), host=platform.platform(),
                                  source_before=snapshot(), git_revision=api.closed_git_revision(root, git_environment(environment), git, root),
                                  cache_before=target_size(target), load_before=host_load())
                    report["toolchain"] = {name: {"path": str(path),
                        **api.RUSTC_PROFILE.stable_file_identity(path),
                        "version": subprocess.check_output([str(path), "-vV"], text=True).strip()}
                        for name, path in (("cargo", cargo), ("rustc", compiler))}
                    record_dir = None
                    if args.measure_compiler:
                        record_dir, report["instrumentation"] = compiler_launcher(state, compiler, api, environment)
                    report["command_environment"] = {key: value for key, value in sorted(environment.items())
                        if key in {"PATH", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "CARGO_INCREMENTAL",
                                   "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
                                   "CARGO_BUILD_TARGET", "CARGO_BUILD_JOBS", "MACOSX_DEPLOYMENT_TARGET", "SOURCE_DATE_EPOCH"}
                        or key.startswith(("CARGO_PROFILE_", "CARGO_TARGET_"))}
                    command = [str(root / "scripts/cargo_fast.sh"), "--stable-local-metadata", "--incremental",
                               "--target-dir", str(target), "--", *arguments]
                    report["command"] = command
                    for index in range(args.warmup + args.repeat):
                        prefix = out.with_name(out.name + f".run-{index + 1}")
                        log, errors = Path(str(prefix) + ".jsonl"), Path(str(prefix) + ".stderr.log")
                        old_records = set(record_dir.iterdir()) if record_dir else set()
                        before_load = host_load()
                        with log.open("x") as stdout, errors.open("x") as stderr:
                            started = time.perf_counter()
                            result = subprocess.run(command, cwd=root, env=environment, stdout=stdout, stderr=stderr)
                            elapsed = time.perf_counter() - started
                        units, fresh, compiled = api.parse_cargo_messages(log.read_text().splitlines())
                        sample = {"elapsed_seconds": elapsed, "returncode": result.returncode,
                                  "fresh": fresh, "compiled": compiled, "units": units,
                                  "load_before": before_load, "load_after": host_load(),
                                  "stdout": str(log), "stderr": str(errors)}
                        # A refused command can exit before Cargo produces timing evidence.
                        group = "warmup" if index < args.warmup else "samples"
                        report[group].append(sample)
                        if result.returncode:
                            raise ValueError(f"local command exited {result.returncode}; see {errors}")
                        sample.update(phase_times(errors.read_text()))
                        messages = cargo_evidence(log.read_text().splitlines())
                        if not units:
                            raise ValueError("Cargo emitted no complete artifact identities")
                        if record_dir:
                            captured = private_directory(Path(str(prefix) + ".compiler-records"))
                            for path in set(record_dir.iterdir()) - old_records:
                                pinned_file(captured / path.name, path.read_bytes(), 0o600)
                            records = api.RUSTC_PROFILE.read_records(captured)
                            sample["compiler_measurements"] = api.RUSTC_PROFILE.reconcile_measurements(
                                [message for message in messages if message.get("reason") == "compiler-artifact"],
                                records, {"source": root, "target": target}, api.artifact_unit,
                                build_script_messages=[message for message in messages if message.get("reason") == "build-script-executed"],
                            )
                    report.update(source_after=snapshot(),
                                  cache_after=target_size(target), load_after=host_load())
                    report["source_unchanged"] = report["source_before"] == report["source_after"]
                    if not report["source_unchanged"]:
                        raise ValueError("local source changed during measurement; samples are diagnostic only")
                    for value in report["toolchain"].values():
                        if api.RUSTC_PROFILE.stable_file_identity(Path(value["path"])) != {key: value[key] for key in ("bytes", "sha256")}:
                            raise ValueError("toolchain changed during measurement")
                    report["summary"] = summarize(report["samples"])
                    report["valid"] = True
            except (OSError, ValueError, subprocess.SubprocessError) as error:
                report.update(valid=False, error=str(error))
            finally:
                json.dump(report, output, indent=2, sort_keys=True)
                output.write("\n")
        print(json.dumps({"report": str(out), "valid": report.get("valid", False), "summary": report.get("summary"), "error": report.get("error")}))
        return 0 if report.get("valid") else 3
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"profile_cargo_build --local: {error}", file=sys.stderr)
        return 3
