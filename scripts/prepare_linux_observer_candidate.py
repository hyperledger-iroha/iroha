#!/usr/bin/env python3
"""Prepare a normal Linux observer publication for independent review, never load it.

Requires Linux x86_64, Python 3.11+, the fixed clean observer checkout, native
Rust 1.99.1 tools and their Cargo home. All paths are explicit; output-root must
not exist and must be outside the source, tools and Cargo home. Dependencies are
fetched before the native publisher's locked offline build. This process may
raise its own descriptor soft limit to retain the complete source census.

Execution trust is the operator-selected fixed source, compiler and CI runner.
Observed hashes are review evidence only, never consumer approval or release
qualification. No signing secrets, bridge loading, or deployment are involved.
"""
from __future__ import annotations

import argparse
from contextlib import ExitStack, contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import resource
import secrets
import string
import stat
import subprocess
import sys

OBSERVER_COMMIT = "3743ff550ca3632e92b23e5d60d084b41a069b29"
SOURCE_HELPER_SHA256 = "35c7710008cc1502dbaaf823eabf0017e0f8d199687943e3d46901e58f94eb97"
NATIVE_TARGET = "x86_64-unknown-linux-gnu"
POLICY_PATH = "crates/iroha_release_bundle/src/bin/iroha_release_bundle/environment-policy.json"
LIBRARY = "libiroha_process_bridge.so"
DIGEST = re.compile(r"[0-9a-f]{64}")


def require(condition, message):
    """Reject before evidence can be promoted into a completed candidate."""
    if not condition:
        raise ValueError(message)


def digest(path):
    """Observe a file digest; this function confers no trust."""
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def canonical(value):
    """Require explicit physical path spelling without normalizing aliases away."""
    path = Path(value)
    require(path.is_absolute() and path == path.resolve() and not any(ord(c) < 32 for c in value),
            "paths must be absolute, canonical and contain no control characters")
    return path


def disjoint(paths):
    """Keep outputs separate from every input and other native output root."""
    for index, first in enumerate(paths):
        for second in paths[index + 1:]:
            require(not first.is_relative_to(second) and not second.is_relative_to(first),
                    "source and output roots must not overlap")


def write_json(path, value):
    """Write new evidence only; never replace a previous attempt."""
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def unique_fields(pairs):
    """Reject ambiguous provenance before describing a completed review packet."""
    result = {}
    for name, value in pairs:
        require(name not in result, "publisher provenance repeats a field")
        result[name] = value
    return result


def run_logged(argv, root, environment, log_root, label, *, pass_fds=()):
    """Own a child synchronously through terminal observation, including errors."""
    stdout = log_root / (label + ".stdout")
    with stdout.open("xb") as out, (log_root / (label + ".stderr")).open("xb") as err:
        child = subprocess.Popen(argv, cwd=root, env=environment, stdout=out, stderr=err,
                                 pass_fds=pass_fds)
        try:
            result = child.wait()
        finally:
            # An interrupted waiter must not release enclosing source originals
            # while its actual child is still running. Never signal that child.
            while child.returncode is None:
                try:
                    child.wait()
                except BaseException:
                    continue
    require(result == 0, f"{label} failed with exit {result}; see retained logs")
    return stdout


def acknowledgement(raw, expected):
    """Accept only the closed normal-publisher protocol and exact output path."""
    require(len(raw) <= 64 * 1024, "publisher acknowledgement exceeds its bound")
    rows = raw.decode("utf-8").split("\n")
    names = ("bridge_dir", "module_sha256", "provenance_sha256")
    require(len(rows) == 4 and rows[-1] == "", "publisher acknowledgement must have three lines")
    values = []
    for row, name in zip(rows, names, strict=False):
        require(row.startswith(name + "\t") and row.count("\t") == 1,
                "publisher acknowledgement has an unexpected field")
        values.append(row[len(name) + 1:])
    require(values[0] == str(expected), "publisher acknowledged a different output directory")
    require(all(DIGEST.fullmatch(value) for value in values[1:]), "publisher digests are malformed")
    return dict(zip(names, values, strict=True))


def reviewed_helper(source):
    """Import the explicitly pinned source-census implementation without bytecode writes."""
    path = source / "scripts/compute_workspace_source_manifest.py"
    require(digest(path) == SOURCE_HELPER_SHA256, "source-census helper differs from fixed observer source")
    spec = importlib.util.spec_from_file_location("observer_candidate_source", path)
    require(spec is not None and spec.loader is not None, "source-census helper is unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@contextmanager
def held_publisher(custody, path):
    """Retain a Cargo build input, including its original stable hardlink count.

    Match the native RetainedBuildInput boundary: retain no-follow ancestors and
    the exact regular executable, then reject every later native snapshot change.
    This grants no private-file/publication authority and changes no source keeper.
    """
    with ExitStack() as stack:
        ancestors = [stack.enter_context(_ancestor(custody, parent)) for parent in reversed(path.parents)]
        before = path.lstat()
        require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
                and before.st_nlink >= 1 and not before.st_mode & 0o7022
                and before.st_mode & 0o111 and 0 < before.st_size <= 2 * 1024**3,
                "publisher build input is not a bounded owner-controlled executable")
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
        original = stack.enter_context(os.fdopen(descriptor, "rb"))
        snapshot = custody._stamp(before)

        def revalidate():
            for ancestor in ancestors:
                ancestor.revalidate()
            require(custody._stamp(os.fstat(descriptor)) == snapshot == custody._stamp(path.lstat()),
                    "publisher build input original or selected name changed")

        revalidate()
        observed = hashlib.file_digest(original, "sha256").hexdigest()
        revalidate()
        yield f"/proc/self/fd/{descriptor}", descriptor, observed
        revalidate()


@contextmanager
def _ancestor(custody, path):
    """Borrow the canonical no-follow directory identity owner without file rules."""
    original = custody._PosixOriginal(path, ancestor=True)
    try:
        yield original
    finally:
        original.close()


def copy_candidate(custody, bridge, destination, pins, source_hash):
    """Copy the two retained originals and cross-check evidence without admitting code."""
    names = (LIBRARY, "publisher-source.json")
    require(set(os.listdir(bridge)) == set(names), "bridge must contain exactly two entries")
    destination.mkdir(mode=0o700)
    with custody.SourceKeeper(bridge, maximum_members=2) as owner:
        owner.capture(names)
        for name, expected in zip(names, (pins["module_sha256"], pins["provenance_sha256"]), strict=True):
            actual = hashlib.sha256()
            with owner.reader(bridge / name) as original, (destination / name).open("xb") as out:
                limit = 512 * 1024 if name == "publisher-source.json" else 2 * 1024**3
                require(0 < os.fstat(original.fileno()).st_size <= limit, "published artifact exceeds its bound")
                while block := original.read(64 * 1024):
                    actual.update(block)
                    out.write(block)
            require(actual.hexdigest() == expected, "published bytes differ from the acknowledgement")
        owner.revalidate()
    raw = (destination / "publisher-source.json").read_bytes()
    require(len(raw) <= 512 * 1024, "publisher provenance exceeds its bound")
    provenance = json.loads(raw, object_pairs_hook=unique_fields)
    require(provenance["schema"] == "iroha.publisher.v1"
            and provenance["source_manifest_sha256"] == source_hash
            and provenance["native_target"] == NATIVE_TARGET
            and provenance["executable_filename"] == LIBRARY
            and provenance["executable_sha256"] == pins["module_sha256"]
            and provenance["executable_bytes"] == (destination / LIBRARY).stat().st_size,
            "publisher provenance differs from the observed candidate")
    return provenance


def prepare(args):
    """Build the fixed-source publisher and create a review packet under one source owner."""
    require(sys.platform.startswith("linux") and platform.machine() == "x86_64", "native Linux x86_64 is required")
    source, output, cargo_home, cargo, rustc = map(canonical, (
        args.source_root, args.output_root, args.cargo_home, args.cargo_bin, args.rustc_bin))
    disjoint([source, output, cargo_home])
    require(not cargo.is_relative_to(output) and not rustc.is_relative_to(output), "tools cannot reside in output")
    require(re.fullmatch(r"[0-9a-f]{40}", args.driver_revision), "driver revision must be an exact commit")
    git = lambda *options: subprocess.check_output(["git", "-C", str(source), *options], text=True).strip()
    require(git("rev-parse", "HEAD") == OBSERVER_COMMIT, "observer checkout is not the fixed reviewed base")
    require(git("status", "--porcelain=v1", "--untracked-files=all") == "", "observer checkout must be clean")
    output.mkdir(mode=0o700)  # Refuse reuse, including an earlier failed invocation.
    for name in ("bootstrap", "cargo", "artifacts", "controls", "review", "logs"):
        (output / name).mkdir(mode=0o700)
    sys.dont_write_bytecode = True
    module = reviewed_helper(source)
    paths = module._git_source_paths(source)
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    needed = len(paths) * 2 + 1024
    require(hard == resource.RLIM_INFINITY or hard >= needed, "descriptor hard limit cannot retain full source")
    if soft < needed:
        resource.setrlimit(resource.RLIMIT_NOFILE, (needed, hard))
    source_hash = module.workspace_source_manifest(source)
    policy = json.loads((source / POLICY_PATH).read_bytes())
    environment = {name: value for name, value in os.environ.items() if name in policy["allowed"]}
    environment["RUSTUP_AUTO_INSTALL"] = "0"
    build_env = dict(environment, CARGO_HOME=str(cargo_home), CARGO_TARGET_DIR=str(output / "bootstrap"),
                     CARGO_INCREMENTAL="0", RUSTC=str(rustc), ENABLE_RANS_BUNDLES="1", NORITO_SKIP_BINDINGS_SYNC="1")
    invocation = "invocation." + "".join(secrets.choice(string.ascii_letters + string.digits) for _ in range(6))
    spec = importlib.util.spec_from_file_location("candidate_source_keeper", source / "scripts/workspace_source_keeper.py")
    require(spec is not None and spec.loader is not None, "source keeper is unavailable")
    custody = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(custody)
    with module.retained_workspace_source(source, source_hash, custody_module=custody) as owner:
        tools = {"cargo": {"path": str(cargo), "sha256": digest(cargo)},
                 "rustc": {"path": str(rustc), "sha256": digest(rustc)}}
        version_file = run_logged([str(rustc), "-vV"], source, build_env, output / "logs", "rustc-version")
        version = version_file.read_text()
        require(version.startswith("rustc 1.99.1 ") and version.splitlines().count("host: " + NATIVE_TARGET) == 1,
                "Rust compiler is not the fixed native Linux toolchain")
        run_logged([str(cargo), "--version"], source, build_env, output / "logs", "cargo-version")
        run_logged([str(cargo), "fetch", "--locked", "--target", NATIVE_TARGET], source, build_env, output / "logs", "fetch")
        run_logged([str(cargo), "build", "--locked", "--offline", "--release", "--target", NATIVE_TARGET,
                    "-p", "iroha_release_bundle", "--features", "dev-tools", "--bin", "iroha_release_bundle"],
                   source, build_env, output / "logs", "publisher-build")
        owner.revalidate()
        publisher = output / "bootstrap" / NATIVE_TARGET / "release/iroha_release_bundle"
        arguments = ["build-bridge-publish", "--repo-root", str(source), "--source-sha256", source_hash,
                     "--cargo-target-dir", str(output / "cargo"), "--artifact-root", str(output / "artifacts"),
                     "--cargo-home", str(cargo_home), "--cargo-bin", str(cargo), "--rustc-bin", str(rustc),
                     "--cancel-request", str(output / "controls/cancel.json"), "--name", invocation]
        with held_publisher(custody, publisher) as (program, descriptor, publisher_sha256):
            stdout = run_logged([program, *arguments], source, environment, output / "logs", "bridge-publication",
                                pass_fds=(descriptor,))
            owner.revalidate()
        bridge = output / "artifacts/sumeragi-release" / source_hash / "bridges" / invocation
        with stdout.open("rb") as stream:
            pins = acknowledgement(stream.read(64 * 1024 + 1), bridge)
        provenance = copy_candidate(custody, bridge, output / "review/bridge", pins, source_hash)
        require(provenance["cargo_lock_sha256"] == digest(source / "Cargo.lock"), "candidate lock differs")
        require(provenance["build_environment_policy_sha256"] == digest(source / POLICY_PATH), "candidate policy differs")
        for name, path in (("cargo", cargo), ("rustc", rustc)):
            require(digest(path) == tools[name]["sha256"] == provenance[name + "_tool"]["sha256"], "native tool identity changed")
        owner.revalidate()
        receipt = {"schema": "iroha.linux-observer-candidate.v1", "status": "requires_independent_approval",
                   "bridge_executed": False, "driver_revision": args.driver_revision,
                   "driver_sha256": digest(Path(__file__)),
                   "observer_commit": OBSERVER_COMMIT, "source_manifest_sha256": source_hash,
                   "native_target": NATIVE_TARGET, "module_sha256": pins["module_sha256"],
                   "provenance_sha256": pins["provenance_sha256"], "publisher_sha256": publisher_sha256,
                   "tools": tools, "kernel": platform.release(), "publisher_arguments": arguments,
                   "source_custody": "retained through build, publication and original artifact copy"}
    # Completion exists only after the source owner's final validation succeeds.
    write_json(output / "review/candidate.json", receipt)
    return receipt


def main():
    """Prepare one candidate; failures leave their original diagnostic logs in place."""
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-root", "output-root", "cargo-home", "cargo-bin", "rustc-bin", "driver-revision"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    receipt = prepare(args)
    print(json.dumps({"status": receipt["status"], "review_directory": str(Path(args.output_root) / "review")}))


if __name__ == "__main__":
    main()
