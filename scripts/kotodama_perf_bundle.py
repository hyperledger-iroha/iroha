#!/usr/bin/env python3
"""Prepare exact Cargo benchmarks and measure their admitted bytes on one VM.

Preparation captures Cargo's real launch recipe through a runner which never
executes the benchmark. Bundles contain no samples. They are same-run CI build
artifacts, not signed releases or a security boundary against repository code.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import sysconfig
import tarfile

SCRIPT = Path(__file__).resolve()
_spec = importlib.util.spec_from_file_location("query_driver", SCRIPT.with_name("kotodama_query_driver.py"))
assert _spec and _spec.loader
QUERY = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(QUERY)
require, sha, read_json, write_json = QUERY.require, QUERY.sha, QUERY.read_json, QUERY.write_json
SCHEMA = "iroha.kotodama.prepared-benchmarks.v1"
CAPTURE_ENV = "IROHA_KOTODAMA_LAUNCH_CAPTURE"
# A closed allowlist: never persist the ambient environment, tokens or credentials.
LAUNCH_KEYS = (
    "CARGO", "CARGO_MANIFEST_DIR", "CARGO_MANIFEST_PATH", "CARGO_CRATE_NAME",
    "CARGO_PKG_NAME", "CARGO_PKG_VERSION", "CARGO_PKG_VERSION_MAJOR",
    "CARGO_PKG_VERSION_MINOR", "CARGO_PKG_VERSION_PATCH", "CARGO_PKG_VERSION_PRE",
    "CARGO_PKG_AUTHORS", "CARGO_PKG_DESCRIPTION", "CARGO_PKG_HOMEPAGE",
    "CARGO_PKG_REPOSITORY", "CARGO_PKG_LICENSE", "CARGO_PKG_LICENSE_FILE",
    "CARGO_PKG_README", "CARGO_PKG_RUST_VERSION", "CARGO_TARGET_TMPDIR",
    "LD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH",
)
BUILD_KEYS = QUERY.BUILD_ENV_KEYS
PROFILE_KEYS = QUERY.PROFILE_ENV_KEYS
RECIPES = {
    "ivm": ("ivm", "bench_kotodama", "crates/ivm/benches/bench_kotodama.rs", []),
    "runtime": ("iroha_core", "kotodama_runtime_cache", "crates/iroha_core/benches/kotodama_runtime_cache.rs", []),
    "queries": ("iroha_core", "queries", "crates/iroha_core/benches/queries.rs", ["--features", "bench,iroha-core-tests"]),
}


def command_output(argv: list[str], **kwargs) -> str:
    return subprocess.check_output(argv, text=True, **kwargs).strip()


def capture_launch() -> None:
    """Record only the real Cargo invocation and explicitly admitted environment."""
    destination = Path(os.environ[CAPTURE_ENV])
    require(len(sys.argv) > 1, "Cargo supplied no benchmark executable")
    write_json(destination, {
        "argv": sys.argv[1:], "cwd": str(Path.cwd()),
        "environment": {key: os.environ[key] for key in LAUNCH_KEYS if key in os.environ},
    })


def cargo_args(name: str) -> list[str]:
    package, bench, _, features = RECIPES[name]
    return ["bench", "--locked", "--jobs", "1", "-p", package, *features, "--bench", bench]


def benchmark_args(name: str, label: str) -> list[str]:
    return (["typed_core_query_"] if name == "queries" else []) + ["--save-baseline", label, "--bench"]


def selected_artifact(messages: list[dict], name: str, root: Path) -> dict:
    package, bench, source, _ = RECIPES[name]
    found = [row for row in messages if row.get("reason") == "compiler-artifact"
             and row.get("target", {}).get("name") == bench]
    require(len(found) == 1, "expected exactly one selected Cargo benchmark artifact")
    row = found[0]
    require(row["target"].get("kind") == ["bench"]
            and row["target"].get("crate_types") == ["bin"]
            and row["target"].get("src_path") == str(root / source)
            and row.get("manifest_path") == str(root / "crates" / package / "Cargo.toml")
            and row.get("profile", {}).get("opt_level") == "3",
            "foreign Cargo benchmark selection")
    return row


def executable_path(artifact: dict, name: str, target: Path) -> Path:
    """Only the exact Cargo-selected benchmark slot may receive prepared bytes."""
    executable = Path(artifact["executable"])
    require(executable.parent == target / "release/deps"
            and re.fullmatch(re.escape(RECIPES[name][1]) + r"-[0-9a-f]{16}", executable.name) is not None,
            "foreign selected executable path")
    return executable


def cargo(argv: list[str], root: Path, environment: dict, name: str) -> dict:
    messages = []
    with subprocess.Popen(["cargo", *argv], cwd=root, env=environment,
                          stdout=subprocess.PIPE, text=True) as child:
        assert child.stdout is not None
        for line in child.stdout:
            print(line, end="", flush=True)
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(row, dict):
                messages.append(row)
        result = child.wait()
    if result:
        raise subprocess.CalledProcessError(result, ["cargo", *argv])
    return selected_artifact(messages, name, root)


def environment_identity() -> dict:
    """Bind the effective build recipe, compiler bytes and native tool versions."""
    QUERY.compiler_environment()
    require(os.environ.get("CARGO_BUILD_JOBS") == "1"
            and os.environ.get("CARGO_INCREMENTAL") == "0"
            and os.environ.get("RUSTUP_TOOLCHAIN") == "1.99.1",
            "benchmark build environment changed")
    require(not os.environ.get("CARGO_BUILD_TARGET")
            and not os.environ.get("RUSTC_WRAPPER")
            and not os.environ.get("RUSTC_WORKSPACE_WRAPPER")
            and not any(key.startswith("CARGO_TARGET_") and key.endswith("_RUNNER")
                        for key in os.environ), "foreign target or compiler/target runner")
    rustc = Path(command_output(["rustup", "which", "rustc"])).resolve(strict=True)
    cargo_path = Path(command_output(["rustup", "which", "cargo"])).resolve(strict=True)
    versions = {name: command_output([name, flag]) for name, flag in
                (("rustc", "-Vv"), ("cargo", "-Vv"), ("cc", "--version"), ("ld", "--version"))}
    require("host: x86_64-unknown-linux-gnu" in versions["rustc"],
            "performance bundles require the hosted Linux x86_64 target")
    return {
        "environment": {key: os.environ[key] for key in (*BUILD_KEYS, *PROFILE_KEYS)
                        if key in os.environ},
        "versions": versions,
        "native_packages": sorted(command_output([
            "dpkg-query", "-W", "-f=${binary:Package}\t${Version}\t${db:Status-Abbrev}\n",
        ]).splitlines()),
        "rustc": str(rustc), "rustc_sha256": sha(rustc),
        "cargo": str(cargo_path), "cargo_sha256": sha(cargo_path),
        "python": str(Path(sys.executable).resolve(strict=True)),
        "python_sha256": sha(Path(sys.executable).resolve(strict=True)),
        "python_libdir": str(Path(sysconfig.get_config_var("LIBDIR")).resolve(strict=True)),
        "image_os": os.environ.get("ImageOS"), "image_version": os.environ.get("ImageVersion"),
        "recipe_sha256": {name: sha(SCRIPT.with_name(name)) for name in
                          ("kotodama_perf_bundle.py", "kotodama_query_driver.py", "check_kotodama_perf.py")},
    }


def source_identity(root: Path) -> dict:
    """Bind the entire tracked source tree without following tracked symlinks."""
    entries = subprocess.check_output(["git", "-C", str(root), "ls-files", "-z"]).split(b"\0")
    digest = hashlib.sha256()
    count = 0
    for raw in sorted(entry for entry in entries if entry):
        relative = raw.decode("utf-8")
        path = root / relative
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode):
            row = [relative, "symlink", os.readlink(path)]
        else:
            require(stat.S_ISREG(mode), "source inventory contains a special file")
            row = [relative, "executable" if mode & 0o111 else "regular", sha(path)]
        digest.update(json.dumps(row, ensure_ascii=True, separators=(",", ":")).encode() + b"\n")
        count += 1
    return {"head": QUERY.policy()["_git_head"](root),
            "tree": command_output(["git", "-C", str(root), "rev-parse", "HEAD^{tree}"]),
            "lock_sha256": sha(root / "Cargo.lock"), "file_count": count,
            "tracked_sha256": digest.hexdigest()}


def run_identity(expected: str) -> dict:
    values = {key: os.environ.get(key, "") for key in
              ("GITHUB_REPOSITORY", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_SHA")}
    require(values["GITHUB_SHA"] == expected
            and re.fullmatch(r"[0-9]+", values["GITHUB_RUN_ID"]) is not None
            and re.fullmatch(r"[0-9]+", values["GITHUB_RUN_ATTEMPT"]) is not None
            and re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", values["GITHUB_REPOSITORY"]) is not None,
            "missing exact workflow run identity")
    return values


def library_identity(executable: Path, environment: dict, toolchain: dict) -> dict:
    """Reject foreign ELF images and bind every resolved native loader dependency."""
    sha(executable)
    with executable.open("rb") as source:
        header = source.read(20)
    require(len(header) == 20 and header[:7] == b"\x7fELF\x02\x01\x01"
            and header[18:20] == b"\x3e\x00", "foreign executable format or architecture")
    output = command_output(["ldd", str(executable)], env=runtime_environment(environment))
    require("not found" not in output, "unresolved runtime library")
    result = {}
    for line in output.splitlines():
        fields = line.strip().split()
        if not fields or fields[0].startswith("linux-vdso.so."):
            continue
        path = fields[2] if len(fields) >= 3 and fields[1] == "=>" else fields[0]
        require(path.startswith("/"), "unrecognized loader dependency")
        resolved = Path(path).resolve(strict=True)
        roots = [Path("/usr/lib"), Path("/usr/lib64"), Path("/lib"), Path("/lib64"),
                 Path(toolchain["rustc"]).parent.parent / "lib", Path(toolchain["python_libdir"])]
        require(any(resolved.is_relative_to(prefix) for prefix in roots),
                "non-system runtime library must not be omitted from a bundle")
        result[fields[0]] = {"path": str(resolved), "sha256": sha(resolved)}
    require(bool(result), "missing native runtime library inventory")
    return result


def runtime_environment(captured: dict) -> dict:
    """Use a closed child environment; ambient credentials and loader hooks stay out."""
    require(set(captured).issubset(LAUNCH_KEYS), "unapproved captured environment key")
    environment = {"PATH": "/usr/bin:/bin", "CARGO_TARGET_DIR": os.environ["CARGO_TARGET_DIR"]}
    environment.update(captured)
    return environment


def validate_launch(launch: dict, name: str, label: str, root: Path, executable: Path,
                    target: Path, toolchain: dict) -> None:
    require(set(launch) == {"argv", "cwd", "environment"}, "foreign launch receipt fields")
    require(launch["argv"] == [str(executable), *benchmark_args(name, label)],
            "Cargo launch arguments changed")
    package = RECIPES[name][0]
    require(launch["cwd"] == str(root / "crates" / package), "Cargo launch directory changed")
    environment = launch["environment"]
    require(set(environment).issubset(LAUNCH_KEYS), "unapproved captured environment key")
    require(environment.get("CARGO_MANIFEST_DIR") == launch["cwd"]
            and environment.get("CARGO_MANIFEST_PATH") == str(root / "crates" / package / "Cargo.toml")
            and environment.get("CARGO_PKG_NAME") == package
            and environment.get("CARGO") == toolchain["cargo"], "Cargo launch package changed")
    # Layout is deliberately identical across all jobs; relocation is not guessed.
    # Empty or relative loader entries would add the current directory implicitly.
    for key in ("LD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH"):
        if key in environment:
            for value in environment[key].split(os.pathsep):
                path = Path(value)
                require(value and path.is_absolute() and ".." not in path.parts
                        and (path.is_relative_to(target)
                             or path.is_relative_to(Path(toolchain["rustc"]).parent.parent / "lib")
                             or str(path) == toolchain.get("python_libdir")),
                        "foreign runtime library search path")


def file_inventory(directory: Path) -> dict:
    result = {}
    for path in sorted(directory.rglob("*")):
        require(not path.is_symlink(), "bundle contains a symlink")
        if path.is_dir():
            continue
        require(path.is_file(), "bundle contains a special file")
        if path == directory / "manifest.json":
            continue
        result[path.relative_to(directory).as_posix()] = {
            "sha256": sha(path), "size": path.stat().st_size,
            "mode": stat.S_IMODE(path.stat().st_mode),
        }
    return result


def prepare(root: Path, candidate: Path, expected: str, label: str, output: Path) -> None:
    root, candidate = root.resolve(strict=True), candidate.resolve(strict=True)
    target = Path(os.environ["CARGO_TARGET_DIR"]).resolve()
    target.mkdir(parents=True, exist_ok=True)
    output = output.resolve()
    require(not output.exists() and output == target / "kotodama-prepared" / label,
            "preparation needs its fresh fixed output directory")
    QUERY.admit_sources(root, candidate, expected, label)
    before = source_identity(root)
    toolchain = environment_identity()
    identity = run_identity(expected)
    output.mkdir(parents=True)
    benches = {}
    for name in RECIPES:
        directory = output / name
        directory.mkdir()
        environment = QUERY.compiler_environment()
        environment.pop("CARGO_BUILD_WARNINGS", None)
        if name == "queries":
            proof = target / "kotodama-query-drivers" / label
            query_dir, plan, artifact = QUERY.prepare(root, candidate, expected, label, proof)
            environment.update({"RUSTC": plan["rustc"], "RUSTC_WRAPPER": str(QUERY.SCRIPT),
                                QUERY.PLAN_ENV: str(query_dir / "plan.json")})
        else:
            argv = [*cargo_args(name), "--no-run", "--message-format=json"]
            artifact = cargo(argv, root, environment, name)
            if artifact.get("fresh") is True:
                disposable = Path(artifact["executable"])
                require(disposable.parent == target / "release/deps"
                        and re.fullmatch(re.escape(RECIPES[name][1]) + r"-[0-9a-f]{16}", disposable.name),
                        "foreign cached executable")
                sha(disposable)
                disposable.unlink()
                artifact = cargo(argv, root, environment, name)
        require(artifact.get("fresh") is False, "missing fresh benchmark compilation")
        executable = executable_path(artifact, name, target)
        original_sha = sha(executable)
        capture = directory / "launch.json"
        environment[CAPTURE_ENV] = str(capture)
        environment["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER"] = str(SCRIPT)
        argv = [*cargo_args(name), "--message-format=json", "--", *benchmark_args(name, label)[:-1]]
        launched = cargo(argv, root, environment, name)
        require(launched.get("fresh") is True and launched["executable"] == str(executable),
                "launch capture rebuilt or selected another executable")
        require(sha(executable) == original_sha, "executable changed during launch capture")
        launch = read_json(capture)
        validate_launch(launch, name, label, root, executable, target, toolchain)
        libraries = library_identity(executable, launch["environment"], toolchain)
        shutil.copy2(executable, directory / "executable")
        require(sha(directory / "executable") == original_sha, "executable changed during packaging")
        benches[name] = {"artifact": artifact, "launch_artifact": launched, "cargo_argv": argv,
                         "sha256": original_sha, "libraries": libraries}
        if name == "queries":
            QUERY.validate_compilation(query_dir, plan, artifact, archived=True)
            shutil.copytree(proof, output / "query-driver")
    QUERY.admit_sources(root, candidate, expected, label)
    require(source_identity(root) == before and environment_identity() == toolchain,
            "source or compiler environment changed during preparation")
    manifest = {"schema": SCHEMA, "label": label, "run": identity, "source": before,
                "root": str(root), "candidate": str(candidate), "target": str(target),
                "toolchain": toolchain, "benches": benches, "files": file_inventory(output)}
    write_json(output / "manifest.json", manifest)
    archive = output.with_suffix(".tar")
    require(not archive.exists(), "preparation archive already exists")
    with tarfile.open(archive, "w") as packed:
        for path in sorted(output.rglob("*")):
            if path.is_file():
                packed.add(path, arcname=path.relative_to(output).as_posix(), recursive=False)
    print(f"Prepared {label}: {archive}", flush=True)


def unpack(archive: Path, output: Path) -> None:
    """Extract only bounded regular relative files; never links or directory escapes."""
    require(not output.exists(), "bundle extraction directory already exists")
    sha(archive)
    with tarfile.open(archive, "r:") as packed:
        members = packed.getmembers()
        seen = set()
        require(0 < len(members) <= 64, "unexpected bundle member count")
        require(sum(member.size for member in members) <= 8 * 1024**3, "bundle is oversized")
        for member in members:
            path = Path(member.name)
            require(member.isfile() and not path.is_absolute() and ".." not in path.parts
                    and path.as_posix() == member.name and member.name not in seen
                    and not member.mode & ~0o777,
                    "unsafe or duplicate bundle archive member")
            seen.add(member.name)
        output.mkdir(parents=True)
        for member in members:
            destination = output / member.name
            destination.parent.mkdir(parents=True, exist_ok=True)
            source = packed.extractfile(member)
            require(source is not None, "missing archive member data")
            with destination.open("xb") as target:
                shutil.copyfileobj(source, target)
            destination.chmod(member.mode)


def admit(directory: Path, root: Path, candidate: Path, expected: str, label: str,
          *, after_measurement: bool = False) -> dict:
    manifest = read_json(directory / "manifest.json")
    target = Path(os.environ["CARGO_TARGET_DIR"]).resolve()
    require(manifest["schema"] == SCHEMA and manifest["label"] == label
            and manifest["run"] == run_identity(expected), "foreign workflow or revision bundle")
    require(manifest["root"] == str(root) and manifest["candidate"] == str(candidate)
            and manifest["target"] == str(target), "bundle workspace layout changed")
    QUERY.admit_sources(root, candidate, expected, label)
    require(manifest["source"] == source_identity(root), "prepared source or lock changed")
    require(manifest["toolchain"] == environment_identity(), "prepared compiler environment changed")
    require(set(manifest["benches"]) == set(RECIPES), "incomplete or duplicate benchmark inventory")
    require(manifest["files"] == file_inventory(directory), "bundle file inventory mismatch")
    prepared = read_json(directory / "query-driver/preparation.json")
    nonce = prepared["nonce"]
    require(re.fullmatch(r"[0-9a-f]{32}", nonce) is not None, "invalid driver nonce")
    allowed = {f"{name}/{file}" for name in RECIPES for file in ("launch.json", "executable")}
    allowed.add("query-driver/preparation.json")
    allowed.update(f"query-driver/{nonce}/{file}" for file in
                   ("queries.rs", "plan.json", "compiler.json", "measured-executable", "measured-dep-info"))
    require(set(manifest["files"]) == allowed, "unexpected bundle files or samples")

    for name in RECIPES:
        row = manifest["benches"][name]
        artifact = selected_artifact([row["artifact"]], name, root)
        launch_artifact = selected_artifact([row["launch_artifact"]], name, root)
        require(artifact.get("fresh") is False and launch_artifact.get("fresh") is True
                and artifact["executable"] == launch_artifact["executable"],
                "bundle has no fresh compilation and real Cargo launch")
        require(row["cargo_argv"] == [*cargo_args(name), "--message-format=json", "--",
                                      *benchmark_args(name, label)[:-1]], "Cargo recipe changed")
        selected = executable_path(artifact, name, target)
        launch = read_json(directory / name / "launch.json")
        validate_launch(launch, name, label, root, selected, target,
                        manifest["toolchain"])
        executable = directory / name / "executable"
        require(sha(executable) == row["sha256"], "prepared executable changed")
        require(library_identity(executable, launch["environment"], manifest["toolchain"]) == row["libraries"],
                "prepared runtime library changed")
    query_output = target / "kotodama-query-drivers" / label
    if not query_output.exists():
        shutil.copytree(directory / "query-driver", query_output)
    actual = file_inventory(query_output)
    measurement = actual.pop("measurement.json", None)
    require((after_measurement or measurement is None)
            and actual == file_inventory(directory / "query-driver"), "stale or changed query evidence")
    QUERY.validate_preparation(query_output, root, candidate, expected, label)
    return manifest


def measure(directory: Path, root: Path, candidate: Path, expected: str, label: str) -> None:
    manifest = admit(directory, root, candidate, expected, label)
    target = Path(os.environ["CARGO_TARGET_DIR"]).resolve()
    query_output = target / "kotodama-query-drivers" / label
    prepared = read_json(query_output / "preparation.json")
    query_dir = query_output / prepared["nonce"]
    plan = read_json(query_dir / "plan.json")
    QUERY.validate_compilation(query_dir, plan, prepared["preparation_artifact"], archived=True)
    for name in RECIPES:
        row = manifest["benches"][name]
        launch = read_json(directory / name / "launch.json")
        archived = (query_dir / "measured-executable" if name == "queries"
                    else directory / name / "executable")
        require(bool(archived.stat().st_mode & stat.S_IXUSR), "prepared executable lost its execute mode")
        require(sha(archived) == row["sha256"], "measurement executable changed")
        executable = executable_path(row["artifact"], name, target)
        executable.parent.mkdir(parents=True, exist_ok=True)
        require(executable.parent.resolve(strict=True) == executable.parent
                and not executable.is_symlink(), "selected executable slot or parent is a symlink")
        # No Cargo build runs on this VM. Each revision keeps its immutable archive;
        # serial installation restores Cargo's exact argv[0] and current_exe location.
        shutil.copy2(archived, executable)
        require(sha(executable) == row["sha256"], "installed executable differs")
        environment = runtime_environment(launch["environment"])
        argv = launch["argv"]
        subprocess.run(argv, cwd=launch["cwd"], env=environment, check=True)
        require(sha(executable) == row["sha256"], "executable changed during timing")
        require(sha(archived) == row["sha256"], "archived executable changed during timing")
        require(library_identity(executable, launch["environment"], manifest["toolchain"]) == row["libraries"],
                "runtime library changed during timing")
        if name == "queries":
            QUERY.validate_compilation(query_dir, plan, prepared["preparation_artifact"], archived=True)
            QUERY.record_measurement(query_output, argv, 0, directory / "manifest.json")
    # Re-admit the complete source, toolchain, bundle and loader after timing too.
    require(admit(directory, root, candidate, expected, label, after_measurement=True) == manifest,
            "prepared bundle changed during measurement")


def main() -> int:
    try:
        if CAPTURE_ENV in os.environ:
            capture_launch()
            return 0
        parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
        parser.add_argument("action", choices=("prepare", "admit", "measure", "unpack"))
        parser.add_argument("--revision-root", type=Path)
        parser.add_argument("--candidate-root", type=Path)
        parser.add_argument("--expected-candidate-commit")
        parser.add_argument("--label", choices=("base", "candidate"))
        parser.add_argument("--output", type=Path, required=True)
        parser.add_argument("--archive", type=Path)
        args = parser.parse_args()
        if args.action == "unpack":
            require(args.archive is not None, "unpack requires an archive")
            unpack(args.archive, args.output)
        else:
            require(all((args.revision_root, args.candidate_root,
                         args.expected_candidate_commit, args.label)), "missing revision admission")
            root, candidate = args.revision_root.resolve(strict=True), args.candidate_root.resolve(strict=True)
            if args.action == "prepare":
                prepare(root, candidate, args.expected_candidate_commit, args.label, args.output)
            else:
                action = admit if args.action == "admit" else measure
                action(args.output.resolve(strict=True), root, candidate,
                       args.expected_candidate_commit, args.label)
        return 0
    except subprocess.CalledProcessError as error:
        return error.returncode
    except (ValueError, OSError, KeyError, RuntimeError, tarfile.TarError) as error:
        print(f"error: prepared benchmark: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
