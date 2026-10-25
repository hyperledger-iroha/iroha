#!/usr/bin/env python3
"""Run a local Cargo loop with the complete maintained native-source preflight.

Used only by cargo_fast.sh --stable-local-metadata on Unix. Requires Python 3.9+,
Git, Cargo and an already compiled matching PLONK build-script evaluator. The
first invocation or a changed evaluator source uses ordinary Cargo to bootstrap;
there is no nested Cargo build, new dependency, source digest override or cache
deletion. The wrapper owns a private lock/stamp under the effective build output.
It checks complete source membership before and after Cargo and leaves unchanged
stamps untouched. Direct Cargo retains its conservative build-script rescan.
Set CARGO_FAST_TIMINGS=1 to emit one diagnostic phase-timing record on stderr;
this does not change source capture, fingerprints or Cargo's exit status.

This is local build-tool coordination, not runtime or release authentication.
Never export CARGO_FAST_NATIVE_SOURCE_CONTEXT manually or use it in sealed builds.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time

CONTEXT = "CARGO_FAST_NATIVE_SOURCE_CONTEXT"
RECEIPT = "native_source_identity.rs"
STAMP_HEADER = "iroha-plonk-local-source-v1"
RECEIPT_HEADER = "iroha-plonk-source-probe-v1"
STAMP_KEYS = {"root", "digest", "watches"}
RECEIPT_KEYS = STAMP_KEYS | {
    "executable", "executable_sha256", "build_script_sha256", "helper_sha256"
}


def sha256(path: Path) -> str:
    """Hash the exact input without loading a complete executable into memory."""
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_record(data: str, header: str, keys: set[str]) -> dict[str, str]:
    """Decode the small closed record emitted by the maintained Rust evaluator."""
    lines = data.splitlines()
    if (len(data.encode()) > 16 * 1024 or "\r" in data or not data.endswith("\n")
            or not lines or lines[0] != header):
        raise ValueError("invalid native source record")
    fields: dict[str, str] = {}
    for line in lines[1:]:
        key, separator, value = line.partition("=")
        if not separator or key in fields or key not in keys or not value:
            raise ValueError("invalid or duplicate native source field")
        if any(character not in "0123456789abcdef" for character in value) or len(value) % 2:
            raise ValueError("noncanonical native source field")
        if key not in {"root", "executable"} and len(value) != 64:
            raise ValueError("invalid native source digest")
        fields[key] = value
    if fields.keys() != keys:
        raise ValueError("incomplete native source record")
    return fields


def record_path(value: str) -> Path:
    """Require a canonical absolute UTF-8 path without symlink aliases."""
    path = Path(bytes.fromhex(value).decode("utf-8"))
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise ValueError("noncanonical native source record path")
    return path


def watch_digest(paths) -> str:
    """Frame the exact maintained watch inventory, matching Rust Path ordering."""
    digest = hashlib.sha256()
    for path in sorted(paths):
        value = os.fsencode(path)
        digest.update(len(value).to_bytes(8, "little"))
        digest.update(value)
    return digest.hexdigest()


def checked_stamp(path: Path, root: Path, digest: str, watched) -> dict[str, str]:
    """Read an existing private stamp and compare it to an independently obtained census."""
    info = owned_regular(path, private=True)
    parent = path.parent
    directory = parent.lstat()
    if (info.st_size > 16 * 1024 or path.name != "snapshot-v1"
            or parent.name != ".iroha-local-source" or not stat.S_ISDIR(directory.st_mode)
            or directory.st_uid != os.geteuid() or stat.S_IMODE(directory.st_mode) != 0o700
            or parent.resolve(strict=True) != parent):
        raise ValueError("unsafe local source stamp directory")
    record = parse_record(path.read_text(), STAMP_HEADER, STAMP_KEYS)
    if (record_path(record["root"]) != root or record["digest"] != digest
            or record["watches"] != watch_digest(watched)):
        raise ValueError("local source stamp differs from the actual source census")
    return record


def owned_regular(path: Path, *, private: bool = False) -> os.stat_result:
    """Reject links, foreign ownership and permissive private metadata."""
    info = path.lstat()
    if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
            or info.st_nlink != 1 or path.resolve(strict=True) != path
            or (private and stat.S_IMODE(info.st_mode) != 0o600)):
        raise ValueError(f"unsafe native source file: {path}")
    return info


def uses_dev_dependencies(arguments: list[str]) -> bool:
    """Skip dev edges only for explicit ordinary Cargo compilation commands."""
    # Prefix options, aliases and other ambiguous command spellings retain the
    # full graph. The wrapper's usual build/check/run/clippy commands start here.
    if not arguments or arguments[0] not in {"build", "check", "run", "clippy"}:
        return True
    for argument in arguments[1:]:
        if argument == "--":
            break  # Program/compiler arguments cannot select Cargo targets.
        if argument.partition("=")[0] in {
            "--all-targets", "--example", "--examples", "--test", "--tests",
            "--bench", "--benches", "--fix",  # Clippy --fix implies --all-targets.
        }:
            return True
    return False


def may_use_plonk(metadata: dict, arguments: list[str], manifest: Path | None) -> bool:
    """Walk selected owners, keeping optional/target edges and relevant dev edges."""
    packages = metadata.get("packages")
    if not isinstance(packages, list):
        return True
    by_name = {package["name"]: package for package in packages}
    by_id = {package["id"]: package for package in packages}
    selected = []
    iterator = iter(arguments)
    for argument in iterator:
        if argument == "--":
            break
        if argument in {"-p", "--package"}:
            selected.append(next(iterator, ""))
        elif argument.startswith("--package="):
            selected.append(argument.partition("=")[2])
        elif argument.startswith("-p") and not argument.startswith("--"):
            selected.append(argument[2:])
    if not selected:
        if "--workspace" in arguments or "--all" in arguments:
            selected = list(by_name)
        elif manifest is not None and manifest.resolve() != Path(metadata["workspace_root"]) / "Cargo.toml":
            selected = [package["name"] for package in packages
                        if Path(package["manifest_path"]).resolve() == manifest.resolve()]
            if not selected:
                return True
        else:
            defaults = metadata.get("workspace_default_members")
            if not isinstance(defaults, list) or any(value not in by_id for value in defaults):
                return True
            selected = [by_id[value]["name"] for value in defaults]
    # Version-qualified, glob or otherwise opaque package selections are left to
    # Cargo. An uncertain selection must keep the complete preflight.
    if any(name not in by_name for name in selected):
        return True
    pending = list(selected)
    visited = set()
    include_dev = uses_dev_dependencies(arguments)
    while pending:
        name = pending.pop()
        if name == "iroha_plonk":
            return True
        if name in visited:
            continue
        visited.add(name)
        pending.extend(dependency["name"] for dependency in by_name[name]["dependencies"]
                       if dependency["name"] in by_name
                       and (include_dev or dependency.get("kind") != "dev"))
    return False


def output_roots(root: Path, arguments: list[str], environment: dict[str, str]) -> list[Path]:
    """Ask Cargo for final/intermediate outputs with the command's config precedence."""
    from check_cargo_target_owner import cargo_selection

    selection = cargo_selection(arguments)
    if selection is None or "clean" in arguments:
        return []
    prefix, manifest, target, build = selection
    selected_environment = environment.copy()
    for key, variable, selected in (("target-dir", "CARGO_TARGET_DIR", target),
                                     ("build-dir", "CARGO_BUILD_BUILD_DIR", build)):
        if selected is not None:
            value = str((root / selected).resolve())
            selected_environment[variable] = value
            prefix.extend(("--config", f"build.{key}={json.dumps(value)}"))
    result = subprocess.run(
        ["cargo", *prefix, "metadata", "--locked", "--offline", "--no-deps",
         "--format-version=1", "--manifest-path", str(root / (manifest or Path("Cargo.toml")))],
        cwd=root, env=selected_environment, check=True, capture_output=True, text=True,
        timeout=30,
    )
    metadata = json.loads(result.stdout)
    if metadata.get("workspace_root") != str(root):
        raise ValueError("local source preflight requires the selected source workspace")
    if not may_use_plonk(metadata, arguments, root / manifest if manifest else None):
        return []
    values = []
    for field in ("build_directory", "target_directory"):
        value = metadata.get(field)
        if not isinstance(value, str) or not Path(value).is_absolute():
            raise ValueError(f"Cargo did not resolve {field}")
        path = Path(value).resolve()
        if path not in values:
            values.append(path)
    return values


def checked_probe(path: Path, root: Path, outputs: list[Path]) -> dict[str, str]:
    """Bind a reusable host evaluator to its exact compiled sources and bytes."""
    info = owned_regular(path)
    if info.st_size > 16 * 1024 or info.st_mode & 0o022:
        raise ValueError("oversized native source evaluator receipt")
    source = path.read_text()
    marker = f"// {RECEIPT_HEADER}\n"
    if source.count(marker) != 1:
        raise ValueError("missing or ambiguous local source evaluator receipt")
    comments = source[source.index(marker):].splitlines(keepends=True)
    if any(not line.startswith("// ") for line in comments):
        raise ValueError("invalid local source evaluator comments")
    record = parse_record("".join(line[3:] for line in comments), RECEIPT_HEADER, RECEIPT_KEYS)
    if record_path(record["root"]) != root:
        raise ValueError("foreign native source evaluator")
    executable = record_path(record["executable"])
    owned_regular(executable)
    if (executable.name != "build-script-build"
            or not executable.parent.name.startswith("iroha_plonk-")
            or executable.parent.parent.name != "build"
            or not any(executable.is_relative_to(output) for output in outputs)):
        raise ValueError("native source evaluator is outside the selected Cargo outputs")
    for source, field in ((root / "crates/iroha_plonk/build.rs", "build_script_sha256"),
                          (root / "crates/iroha_plonk/build_support/source_identity.rs", "helper_sha256"),
                          (executable, "executable_sha256")):
        if sha256(source) != record[field]:
            raise ValueError("native source evaluator inputs changed")
    return record


def find_probe(root: Path, outputs: list[Path]) -> tuple[Path, dict[str, str]] | None:
    """Select a verified existing host evaluator without compiling or repairing it."""
    candidates = set()
    for output in outputs:
        for pattern in (f"*/build/iroha_plonk-*/out/{RECEIPT}",
                        f"*/*/build/iroha_plonk-*/out/{RECEIPT}"):
            candidates.update(output.glob(pattern))
    for path in sorted(candidates):
        try:
            return path, checked_probe(path, root, outputs)
        except (OSError, ValueError, UnicodeError):
            # Old sources, incomplete interrupted writes and unsafe receipts
            # never become authority. Ordinary Cargo safely regenerates them.
            continue
    return None


def evaluate(probe: tuple[Path, dict[str, str]], root: Path, outputs: list[Path]) -> str:
    """Run the exact Rust census, checking its executable and inputs on both sides."""
    path, expected = probe
    if checked_probe(path, root, outputs) != expected:
        raise ValueError("native source evaluator changed before preflight")
    try:
        result = subprocess.run(
            [str(record_path(expected["executable"])), "--source-snapshot", str(root)],
            cwd=root, check=True, capture_output=True, text=True,
        )
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or "").strip()
        raise ValueError(
            f"native source evaluator exited {error.returncode}"
            + (f": {detail}" if detail else "")
        ) from error
    value = parse_record(result.stdout, STAMP_HEADER, STAMP_KEYS)
    if record_path(value["root"]) != root:
        raise ValueError("native source evaluator returned another workspace")
    if checked_probe(path, root, outputs) != expected:
        raise ValueError("native source evaluator changed during preflight")
    return result.stdout


def private_directory(output: Path) -> Path:
    """Create only the wrapper's named private state directory, never replace it."""
    output.mkdir(parents=True, exist_ok=True)
    directory = output / ".iroha-local-source"
    try:
        directory.mkdir(mode=0o700)
    except FileExistsError:
        pass
    info = directory.lstat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid()
            or stat.S_IMODE(info.st_mode) != 0o700 or directory.resolve(strict=True) != directory):
        raise ValueError("unsafe local source state directory")
    return directory


def update_stamp(path: Path, contents: str) -> None:
    """Keep an identical source stamp's inode/mtime, rejecting unsafe old files."""
    try:
        owned_regular(path, private=True)
        if path.read_text() == contents:
            return
    except FileNotFoundError:
        pass
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "w") as stream:
        info = os.fstat(stream.fileno())
        if (info.st_uid != os.geteuid() or info.st_nlink != 1
                or not stat.S_ISREG(info.st_mode)
                or stat.S_IMODE(info.st_mode) != 0o600):
            raise ValueError("unsafe local source stamp")
        stream.truncate(0)
        stream.write(contents)
        stream.flush()
        os.fsync(stream.fileno())


@contextmanager
def phase(timings, name):
    """Optionally record diagnostic elapsed time, including a refused phase."""
    started = time.perf_counter()
    try:
        yield
    finally:
        if timings is not None:
            timings[name] = timings.get(name, 0.0) + time.perf_counter() - started


def run(root: Path, driver: str, arguments: list[str], timings=None) -> int:
    """Run one locally coordinated Cargo invocation and reject source drift."""
    import fcntl

    environment = os.environ.copy()
    environment.pop(CONTEXT, None)
    if (environment.get("VERGEN_GIT_SHA") != "local-fast-build"
            or "IROHA_GIT_COMMIT_HASH" in environment):
        raise ValueError("native source preflight requires unsealed local-fast-build metadata")
    with phase(timings, "discovery_seconds"):
        outputs = output_roots(root, arguments, environment)
    if not outputs:
        with phase(timings, "cargo_seconds"):
            return subprocess.call([driver, *arguments], cwd=root, env=environment, close_fds=False)
    with phase(timings, "receipt_seconds"):
        initial_probe = find_probe(root, outputs)
    if initial_probe is None:
        # Small unrelated packages need no workspace census, and a first PLONK
        # build can register its evaluator through ordinary Cargo on its own.
        print("[cargo-fast] native source preflight: no matching evaluator; using conservative Cargo checks", file=sys.stderr)
        with phase(timings, "cargo_seconds"):
            return subprocess.call([driver, *arguments], cwd=root, env=environment, close_fds=False)
    directory = private_directory(outputs[0])
    lock_path = directory / "lock"
    descriptor = os.open(lock_path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "r+") as lock:
        owned_regular(lock_path, private=True)
        # Wait for another local command naturally; never signal or replace it.
        with phase(timings, "lock_seconds"):
            fcntl.flock(lock, fcntl.LOCK_EX)
        with phase(timings, "receipt_seconds"):
            probe = find_probe(root, outputs)
        before = None
        if probe is not None:
            with phase(timings, "preflight_before_seconds"):
                before = evaluate(probe, root, outputs)
                stamp = directory / "snapshot-v1"
                update_stamp(stamp, before)
                environment[CONTEXT] = str(stamp)
        else:
            print("[cargo-fast] native source preflight: bootstrapping with conservative Cargo checks", file=sys.stderr)
        # As with the previous shell exec, preserve already inheritable Cargo
        # jobserver descriptors. Python's own private lock fd stays close-on-exec.
        with phase(timings, "cargo_seconds"):
            result = subprocess.call([driver, *arguments], cwd=root, env=environment, close_fds=False)
        if probe is not None:
            # Cargo may refresh its receipt without changing the evaluator.
            with phase(timings, "preflight_after_seconds"):
                refreshed = checked_probe(probe[0], root, outputs)
                after = evaluate((probe[0], refreshed), root, outputs)
                if before != after:
                    raise ValueError("native source inputs changed during Cargo; rerun on stable source")
        return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--cargo-driver", default="cargo")
    parser.add_argument("cargo_args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    arguments = args.cargo_args[1:] if args.cargo_args[:1] == ["--"] else args.cargo_args
    if not arguments:
        parser.error("supply Cargo arguments after --")
    timings = {} if os.environ.get("CARGO_FAST_TIMINGS") == "1" else None
    try:
        return run(args.source_root.resolve(strict=True), args.cargo_driver, arguments, timings)
    except (OSError, ValueError, UnicodeError, subprocess.SubprocessError) as error:
        print(f"cargo-fast: native source preflight failed: {error}", file=sys.stderr)
        return 3
    finally:
        if timings is not None:
            print("[cargo-fast-timings] " + json.dumps(timings, sort_keys=True), file=sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
