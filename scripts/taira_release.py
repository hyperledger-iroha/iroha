#!/usr/bin/env python3
"""Check and prepare local Taira binaries without deployment authority.

Requires Python 3.11+, Git, the repository Rust toolchain, a warm Cargo target,
and explicitly hash-pinned Zig/cargo-zigbuild executables. `check` runs the
maintained native CLI gate; `prepare` also builds the four Linux release binaries
with six jobs and captures read-only copies in a fresh output directory.
No keys, runtime configuration, SSH, signing, activation or publishing inputs
are accepted. Output is a local build observation, not release qualification.
Existing source, outputs and Cargo caches are never overwritten or cleaned.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import struct
import subprocess
import sys
import time

sys.dont_write_bytecode = True
from release_artifact_contract import (
    ReleaseArtifactError, canonical_json_bytes, create_fresh_directory,
    exclusive_output_fd, exclusive_write_bytes, stable_hash_path,
    stable_open_relative,
)
import taira_release_check as gate


TARGET = "aarch64-unknown-linux-gnu"
BINARIES = (("iroha3d_taira", "irohad"), ("iroha", "iroha_cli"),
            ("sorafs-node", "sorafs_node"), ("kagami", "iroha_kagami"))
MAX_BINARY_BYTES = 4 * 1024**3
BUILD_SOURCES = ("scripts/taira_release.py", "scripts/taira_release_check.py",
                 "scripts/release_artifact_contract.py", "scripts/cargo_fast.sh",
                 "scripts/cargo_zigbuild_linux.sh", "scripts/zig_linux_gnu.py")


class PrepareError(RuntimeError):
    """Local preparation stopped before publishing a successful result."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PrepareError(message)


def real_path(path: Path, *, exists: bool = True) -> Path:
    require(path.is_absolute() and Path(os.path.abspath(path)) == path,
            "paths must be absolute and normalized")
    require(path.resolve(strict=exists) == path, "paths must not contain symlinks")
    return path


def child_environment(inherited: dict[str, str], target_dir: Path) -> dict[str, str]:
    # Do not forward runtime secrets, compiler overrides or interpreter hooks.
    allowed = {"PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "TMPDIR", "TMP", "TEMP",
               "SCCACHE_DIR", "SCCACHE_CACHE_SIZE"}
    env = {key: value for key, value in inherited.items() if key in allowed}
    env.update(LC_ALL="C", PYTHONNOUSERSITE="1", PYTHONDONTWRITEBYTECODE="1",
               CARGO_TARGET_DIR=str(target_dir))
    return env


def git(root: Path, *args: str) -> bytes:
    result = subprocess.run(["git", "--no-replace-objects", *args], cwd=root, stdin=subprocess.DEVNULL,
                            capture_output=True, check=False, timeout=60,
                            env=child_environment(dict(os.environ), root / "target"))
    require(result.returncode == 0, "git " + args[0] + " failed")
    return result.stdout.strip()


def verify_checkout(root: Path, commit: str, expected_signer: str) -> str:
    require(re.fullmatch(r"[0-9a-f]{40}", commit) is not None,
            "expected commit must be a full lowercase Git object ID")
    require(re.fullmatch(r"(?:[0-9A-F]{40}|[0-9A-F]{64}|SHA256:[A-Za-z0-9+/]{43})", expected_signer) is not None,
            "expected signer must be a full signing-key fingerprint")
    require(git(root, "rev-parse", "--show-toplevel") == os.fsencode(root),
            "repository root does not match the checkout")
    require(git(root, "branch", "--show-current") == b"optimizations",
            "Taira preparation requires optimizations")
    require(git(root, "rev-parse", "HEAD").decode() == commit,
            "HEAD differs from the expected commit")
    require(not git(root, "status", "--porcelain=v1", "--untracked-files=all", "--ignore-submodules=none"), "Taira preparation requires clean source")
    git(root, "verify-commit", commit)
    require(git(root, "show", "--no-patch", "--format=%GF", commit).decode() == expected_signer,
            "commit signature does not match the expected signer")
    for path in BUILD_SOURCES:
        git(root, "ls-files", "--error-unmatch", "--", path)
    return git(root, "rev-parse", "HEAD^{tree}").decode()


def file_identity(info: os.stat_result) -> tuple[int, ...]:
    return (info.st_dev, info.st_ino, info.st_mode, info.st_nlink, info.st_uid,
            info.st_gid, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def source_snapshot(root: Path) -> list[dict[str, object]]:
    rows = []
    for raw in git(root, "ls-files", "--stage", "-z").split(b"\0"):
        if not raw:
            continue
        entry = re.fullmatch(rb"(100644|100755|120000|160000) ([0-9a-f]{40}) 0\t(.+)", raw, re.DOTALL)
        require(entry is not None, "tracked index entry has unsupported mode, object or merge stage")
        mode, oid, relative_raw = entry.groups()
        relative = os.fsdecode(relative_raw)
        require(not Path(relative).is_absolute() and ".." not in Path(relative).parts,
                "tracked index path must stay inside the checkout")
        path = root / relative
        if mode == b"160000":
            # No submodule source is imported by this release corridor. Bind the
            # indexed commit and prove its local path contributes no build input.
            try:
                before = path.lstat()
            except FileNotFoundError:
                require(not os.path.lexists(path), "gitlink path appeared during inspection")
                checkout = "absent"
            else:
                require(stat.S_ISDIR(before.st_mode) and not any(path.iterdir()),
                        "gitlinks must be uninitialized empty directories or absent")
                require(file_identity(path.lstat()) == file_identity(before),
                        "gitlink changed during inspection")
                checkout = "empty"
            rows.append({"path": relative, "kind": "gitlink", "index_mode": mode.decode(),
                         "object": oid.decode(), "uninitialized": True, "checkout": checkout})
            continue
        before = path.lstat()
        if stat.S_ISLNK(before.st_mode):
            require(mode == b"120000", "tracked source kind differs from the index: " + relative)
            payload = os.fsencode(os.readlink(path))
            blob = hashlib.sha1(f"blob {len(payload)}\0".encode() + payload).hexdigest()
            digest, size, kind = hashlib.sha256(payload).hexdigest(), len(payload), "symlink"
        else:
            require(stat.S_ISREG(before.st_mode), "tracked source must be regular files or symlinks")
            require(mode in (b"100644", b"100755")
                    and bool(before.st_mode & stat.S_IXUSR) == (mode == b"100755"),
                    "tracked source mode differs from the index: " + relative)
            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
            with os.fdopen(fd, "rb") as stream:
                require(file_identity(os.fstat(stream.fileno())) == file_identity(before),
                        "source changed before hashing")
                sha256 = hashlib.sha256()
                git_blob = hashlib.sha1(f"blob {before.st_size}\0".encode())
                while block := stream.read(1024 * 1024):
                    sha256.update(block)
                    git_blob.update(block)
                digest, blob = sha256.hexdigest(), git_blob.hexdigest()
                require(file_identity(os.fstat(stream.fileno())) == file_identity(before),
                        "source changed while hashing")
            size, kind = before.st_size, "regular"
        require(file_identity(path.lstat()) == file_identity(before), "source path changed while hashing")
        # Git status can conceal edits marked assume-unchanged/skip-worktree.
        # Compare raw bytes to the indexed Git blob during the same bounded read.
        require(blob == oid.decode(), "tracked source bytes differ from the index: " + relative)
        rows.append({"path": relative, "kind": kind, "size": size,
                     "index_mode": mode.decode(), "object": oid.decode(),
                     "mode": stat.S_IMODE(before.st_mode), "sha256": digest})
    return rows


def verify_tool(path: Path, expected: str) -> dict[str, object]:
    real_path(path)
    require(re.fullmatch(r"[0-9a-f]{64}", expected) is not None, "tool digest must be lowercase SHA256")
    info = stable_hash_path(path)
    require(info.sha256 == expected and bool(info.mode & stat.S_IXUSR),
            "tool is not the exact reviewed executable: " + path.name)
    return {"path": str(path), "sha256": info.sha256, "size": info.size}


def build_command(root: Path, target_dir: Path) -> list[str]:
    command = [str(root / "scripts/cargo_zigbuild_linux.sh"), "--target-dir", str(target_dir),
               "--linker", "off", "--jobs", "6", "--", "zigbuild", "--locked",
               "--profile", "release", "--target", TARGET]
    for name, package in BINARIES:
        command.extend(("-p", package, "--bin", name))
    return command


def run_build(root: Path, command: list[str], env: dict[str, str], log: Path) -> None:
    failure = None
    # Commit diagnostic output even on compiler failure; the result is published
    # only after a successful build and independent source/tool revalidation.
    with exclusive_output_fd(log, mode=0o600) as output:
        try:
            child = subprocess.Popen(command, cwd=root, env=env, stdin=subprocess.DEVNULL,
                                     stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                require(child.wait() == 0, "Linux build failed; inspect " + str(log))
            except BaseException:
                if child.poll() is None:
                    try:
                        os.killpg(child.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                    try:
                        child.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        try:
                            os.killpg(child.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        child.wait(timeout=10)
                raise
        except BaseException as error:
            failure = error
    if failure is not None:
        raise failure


def valid_elf(header: bytes) -> bool:
    return (len(header) >= 20 and header[:7] == b"\x7fELF\x02\x01\x01"
            and struct.unpack("<H", header[16:18])[0] in (2, 3)
            and struct.unpack("<H", header[18:20])[0] == 183)


def freeze(path: Path, *, directory: bool = False) -> None:
    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC
    if directory:
        flags |= os.O_DIRECTORY
    fd = os.open(path, flags)
    try:
        info = os.fstat(fd)
        require(info.st_uid == os.geteuid() and
                (info.st_dev, info.st_ino) == (path.lstat().st_dev, path.lstat().st_ino),
                "capture ownership or path changed")
        os.fchmod(fd, 0o500 if directory or info.st_mode & stat.S_IXUSR else 0o400)
        os.fsync(fd)
    finally:
        os.close(fd)


def capture_artifacts(target_dir: Path, output_dir: Path) -> list[dict[str, object]]:
    capture = create_fresh_directory(output_dir / "bin", mode=0o700)
    rows = []
    for name, package in BINARIES:
        relative = f"{TARGET}/release/{name}"
        original = target_dir / relative
        expected = stable_hash_path(original, max_size=MAX_BINARY_BYTES)
        require(expected.size >= 20 and bool(expected.mode & stat.S_IXUSR),
                "release artifact must be an executable ELF")
        require(original.stat().st_uid == os.geteuid(), "release artifact must be owner-held")
        destination = capture / name
        with stable_open_relative(target_dir, relative, expected=expected) as source:
            require(valid_elf(os.read(source, 20)), "release artifact is not AArch64 Linux ELF: " + name)
            os.lseek(source, 0, os.SEEK_SET)
            with exclusive_output_fd(destination, mode=0o755) as output:
                digest, size = hashlib.sha256(), 0
                while block := os.read(source, 1024 * 1024):
                    size += len(block)
                    require(size <= expected.size, "artifact grew during capture")
                    digest.update(block)
                    view = memoryview(block)
                    while view:
                        written = os.write(output, view)
                        require(written > 0, "artifact capture made no write progress")
                        view = view[written:]
                require(size == expected.size and digest.hexdigest() == expected.sha256,
                        "artifact changed during capture")
        freeze(destination)
        actual = stable_hash_path(destination, max_size=MAX_BINARY_BYTES)
        require(actual.sha256 == expected.sha256 and actual.mode == 0o500,
                "read-only artifact capture differs from the build")
        rows.append({"name": name, "package": package, "path": str(destination),
                     "sha256": actual.sha256, "size": actual.size})
    freeze(capture, directory=True)
    return rows


def prepare(args: argparse.Namespace) -> dict[str, object]:
    root, target_dir = real_path(args.repo_root), real_path(args.target_dir)
    output = real_path(args.output_dir, exists=False)
    require(Path(__file__).resolve() == root / "scripts/taira_release.py",
            "prepare must use the maintained script from the selected checkout")
    require(target_dir.is_dir(), "target-dir must be an existing warm Cargo lane")
    require(not output.exists(), "output-dir must be fresh")
    if output.is_relative_to(root):
        require(output.is_relative_to(root / "target"), "repository outputs must stay under target/")
    require(output != target_dir and not target_dir.is_relative_to(output),
            "output-dir must not contain the Cargo lane")
    tree = verify_checkout(root, args.expected_commit, args.expected_signer)
    before = source_snapshot(root)
    env = child_environment(dict(os.environ), target_dir)
    tools = [verify_tool(args.zig, args.zig_sha256),
             verify_tool(args.cargo_zigbuild, args.cargo_zigbuild_sha256)]
    selected = shutil.which("cargo-zigbuild", path=env.get("PATH", ""))
    require(selected is not None and Path(selected).resolve() == args.cargo_zigbuild,
            "PATH must select the explicitly pinned cargo-zigbuild")
    env.update(IROHA_ZIG_BINARY=str(args.zig), IROHA_GIT_COMMIT_HASH=args.expected_commit,
               VERGEN_GIT_SHA=args.expected_commit)
    output = create_fresh_directory(output, mode=0o700)
    timings: dict[str, float] = {}

    def stage(label, operation):
        print(f"[taira-release] start {label}", flush=True)
        started = time.monotonic()
        try:
            return operation()
        finally:
            timings[label] = round(time.monotonic() - started, 3)
            print(f"[taira-release] {label} elapsed {timings[label]:.3f}s", flush=True)

    def revalidate():
        require(verify_checkout(root, args.expected_commit, args.expected_signer) == tree and source_snapshot(root) == before,
                "signed source changed during preparation")
        require([verify_tool(args.zig, args.zig_sha256),
                 verify_tool(args.cargo_zigbuild, args.cargo_zigbuild_sha256)] == tools,
                "toolchain changed during preparation")

    stage("native CLI checks", lambda: gate.run_checks(root, environment=env))
    revalidate()
    command = build_command(root, target_dir)
    stage("Linux release build", lambda: run_build(root, command, env, output / "cargo.log"))
    revalidate()
    artifacts = stage("read-only artifact capture", lambda: capture_artifacts(target_dir, output))
    revalidate()
    result = {"commit": args.expected_commit, "signer_fingerprint": args.expected_signer, "tree": tree, "target": TARGET, "profile": "release",
              "jobs": 6, "source_unchanged": True, "toolchain_unchanged": True,
              "source_snapshot_sha256": hashlib.sha256(canonical_json_bytes(before)).hexdigest(),
              "tools": tools, "command": command, "artifacts": artifacts, "timings_seconds": timings,
              "release_qualified": False, "deployed": False}
    exclusive_write_bytes(output / "result.json", canonical_json_bytes(result), mode=0o600)
    freeze(output / "cargo.log")
    freeze(output / "result.json")
    freeze(output, directory=True)
    return result


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    for name in ("check", "prepare"):
        command = commands.add_parser(name)
        command.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
        command.add_argument("--target-dir", type=Path, help="existing warm Cargo lane (default: repo target/)")
        if name == "prepare":
            command.add_argument("--expected-commit", required=True)
            command.add_argument("--expected-signer", required=True, help="independently reviewed signing-key fingerprint")
            command.add_argument("--output-dir", type=Path, required=True)
            command.add_argument("--zig", type=Path, required=True, help="absolute real Zig executable")
            command.add_argument("--zig-sha256", required=True)
            command.add_argument("--cargo-zigbuild", type=Path, required=True, help="absolute real cargo-zigbuild executable")
            command.add_argument("--cargo-zigbuild-sha256", required=True)
    return result


def main() -> int:
    args = parser().parse_args()
    args.target_dir = args.target_dir or args.repo_root / "target"
    try:
        require(sys.platform in {"darwin", "linux"}, "Taira preparation requires macOS or Linux")
        if args.command == "check":
            target_dir = real_path(args.target_dir)
            require(target_dir.is_dir(), "target-dir must be an existing warm Cargo lane")
            gate.run_checks(real_path(args.repo_root), environment=child_environment(dict(os.environ), target_dir))
        else:
            prepared = prepare(args)
            print(f"[taira-release] prepared {prepared['commit']}: {args.output_dir / 'result.json'}", flush=True)
    except (PrepareError, ReleaseArtifactError, gate.CheckError, OSError, subprocess.SubprocessError) as error:
        print(f"[taira-release] FAIL: {error}", file=sys.stderr, flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
