#!/usr/bin/env python3
"""Build a normal observer publication and run the complete Linux harness suite.

The CI invoker supplies a reviewed clean commit and installed Cargo/Rustc paths.
Dependencies must already be fetched for the native host. The kernel must support
the normal publisher's sealed executable images and unprivileged own-process perf
observation; this controller changes neither host policy nor observer acceptance.
All build outputs and retained logs use a fresh private external --run-root.

Authority flows from the reviewed source seal and installed tools, through exact
Cargo publisher build events and its retained executable, to the normal publisher
receipt. Tests receive those original receipt pins, never hashes discovered from
a selected shared library. This is component CI, not deployment qualification.
"""

from __future__ import annotations

import argparse
from contextlib import ExitStack, contextmanager
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys


def require(condition: bool, message: str) -> None:
    """Reject incomplete producer evidence before admitting a consumer."""
    if not condition:
        raise ValueError(message)


def executable(path: Path) -> Path:
    """Require an explicit canonical executable, without resolving an alias."""
    require(path.is_absolute() and path.resolve(strict=True) == path,
            "tool path must be absolute and canonical")
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode)
            and os.access(path, os.X_OK), "tool must be an executable regular file")
    return path


def publisher_package(raw: bytes, repository: Path) -> str:
    """Bind Cargo's package identity to the reviewed publisher manifest and binary."""
    require(len(raw) <= 8 * 1024 * 1024, "publisher metadata exceeds its bound")
    metadata = json.loads(raw)
    require(type(metadata) is dict and metadata.get("version") == 1,
            "publisher requires Cargo metadata format one")
    packages = [package for package in metadata["packages"]
                if package.get("name") == "iroha_release_bundle"]
    require(len(packages) == 1, "publisher metadata must select exactly one package")
    package = packages[0]
    root = repository / "crates/iroha_release_bundle"
    targets = [target for target in package["targets"]
               if target.get("name") == "iroha_release_bundle" and target.get("kind") == ["bin"]]
    require(package.get("manifest_path") == str(root / "Cargo.toml")
            and package.get("source") is None and len(targets) == 1
            and targets[0].get("src_path") == str(root / "src/bin/iroha_release_bundle.rs")
            and targets[0].get("crate_types") == ["bin"]
            and targets[0].get("required-features") == ["dev-tools"]
            and isinstance(package.get("id"), str) and 0 < len(package["id"]) <= 8192,
            "publisher metadata does not bind the exact reviewed source target")
    return package["id"]


def publisher_from_events(raw: bytes, repository: Path, target: Path, native: str,
                          package_id: str) -> Path:
    """Authenticate the fresh publisher output against the exact requested Cargo target."""
    require(len(raw) <= 32 * 1024 * 1024, "publisher Cargo event stream exceeds its bound")
    events = [json.loads(line) for line in raw.splitlines()]
    require(events and all(type(event) is dict and event.get("reason") in (
        "compiler-artifact", "compiler-message", "build-script-executed", "build-finished"
    ) for event in events), "publisher emitted an unsupported Cargo event")
    finished = [event for event in events if event.get("reason") == "build-finished"]
    require(finished == [{"reason": "build-finished", "success": True}]
            and events[-1] == finished[0],
            "publisher needs exactly one successful terminal Cargo event")
    candidates = [event for event in events if event.get("reason") == "compiler-artifact"
                  and event.get("target", {}).get("name") == "iroha_release_bundle"
                  and event.get("executable") is not None]
    require(len(candidates) == 1, "publisher build must emit exactly one matching executable")
    event = candidates[0]
    package = repository / "crates/iroha_release_bundle"
    expected = target / native / "debug/iroha_release_bundle"
    require(event.get("package_id") == package_id
            and event.get("manifest_path") == str(package / "Cargo.toml")
            and event["target"].get("src_path") == str(package / "src/bin/iroha_release_bundle.rs")
            and event["target"].get("kind") == ["bin"]
            and event["target"].get("crate_types") == ["bin"]
            and event.get("features") == ["dev-tools"]
            and event.get("profile", {}).get("test") is False
            and event.get("fresh") is False
            and event.get("executable") == str(expected)
            and str(expected) in event.get("filenames", []),
            "publisher output does not bind the exact reviewed source/build request")
    return expected


def bridge_receipt(raw: bytes, expected: Path) -> dict[str, str]:
    """Accept only the normal publisher's complete original three-field receipt."""
    require(len(raw) <= 64 * 1024, "bridge receipt exceeds its bound")
    lines = raw.decode("utf-8").splitlines(keepends=True)
    require(len(lines) == 3 and all(line.endswith("\n") for line in lines),
            "bridge receipt must contain exactly three terminated fields")
    fields = [line[:-1].split("\t") for line in lines]
    require(all(len(field) == 2 for field in fields)
            and [field[0] for field in fields] == ["bridge_dir", "module_sha256", "provenance_sha256"],
            "bridge receipt has a different field inventory")
    result = dict(fields)
    require(result["bridge_dir"] == str(expected), "publisher returned another invocation's artifact")
    require(all(re.fullmatch(r"[0-9a-f]{64}", result[name])
                for name in ("module_sha256", "provenance_sha256")),
            "publisher receipt pins must be lowercase SHA-256")
    return result


@contextmanager
def retained_executable(path: Path):
    """Yield the held original descriptor and an explicit custody revalidation fence."""
    executable(path)
    before = path.lstat()
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    fields = ("st_dev", "st_ino", "st_mode", "st_uid", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    def identity(value):
        """Compare the held original and its selected name without access-time drift."""
        return tuple(getattr(value, field) for field in fields)

    def revalidate():
        """Refuse a changed tool or selected name before accepting its outputs."""
        require(identity(os.fstat(descriptor)) == identity(before)
                and identity(path.lstat()) == identity(before), "executable original changed during use")

    try:
        revalidate()
        yield descriptor, revalidate
        revalidate()
    finally:
        os.close(descriptor)


def run(args: argparse.Namespace) -> None:
    """Retain the approved source across publisher construction, publication and all tests."""
    require(sys.platform.startswith("linux"), "native observer CI requires Linux")
    repository = args.source_root
    require(repository.is_absolute() and repository.resolve(strict=True) == repository,
            "source root must be canonical")
    cargo, rustc = executable(args.cargo_bin), executable(args.rustc_bin)
    require(args.cargo_home.is_absolute() and args.cargo_home.resolve(strict=True) == args.cargo_home,
            "Cargo home must be canonical")
    subprocess.run([sys.executable, "-I", "-S", str(repository / "scripts/check_release_feature_graph.py"),
                    "--validate-source-commit", args.source_commit], cwd=repository, check=True)
    clean = subprocess.check_output(["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=repository)
    require(not clean, "normal observer publication requires an exact clean checkout")

    # The bootstrap invoker owns a closed build environment; it never forwards
    # setup-action compiler overrides into the maintained publisher's policy.
    policy = json.loads((repository / "scripts/environment-policy.json").read_text())
    environment = {name: os.environ[name] for name in policy["allowed"] if name in os.environ}
    environment.update(CARGO_HOME=str(args.cargo_home), RUSTUP_AUTO_INSTALL="0")
    version = subprocess.check_output([str(rustc), "-vV"], env=environment, text=True)
    hosts = [line[6:] for line in version.splitlines() if line.startswith("host: ")]
    require(len(hosts) == 1 and re.fullmatch(r"[a-z0-9_]+-unknown-linux-gnu", hosts[0]),
            "publisher must use an actual native Linux GNU toolchain")
    native = hosts[0]
    require(args.run_root.is_absolute() and args.run_root.parent.resolve(strict=True) == args.run_root.parent,
            "run root must have a canonical existing parent")
    require(not args.run_root.is_relative_to(repository) and not args.run_root.is_relative_to(args.cargo_home),
            "private build outputs must be external to source and Cargo home")
    args.run_root.mkdir(mode=0o700)
    roots = {name: args.run_root / name for name in ("bootstrap", "cargo", "artifacts", "evidence")}
    for path in roots.values():
        path.mkdir(mode=0o700)
    evidence = roots["evidence"]
    spec = importlib.util.spec_from_file_location("ci_source_owner", repository / "scripts/compute_workspace_source_manifest.py")
    require(spec is not None and spec.loader is not None, "canonical source owner is unavailable")
    source_module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = source_module
    spec.loader.exec_module(source_module)
    manifest = source_module.native_artifact_workspace_source_manifest(repository)

    with ExitStack() as owners:
        # Installed tools stay retained across the bootstrap and normal producer;
        # the normal producer additionally enforces its native tool-image policy.
        _, revalidate_cargo = owners.enter_context(retained_executable(cargo))
        _, revalidate_rustc = owners.enter_context(retained_executable(rustc))
        source = owners.enter_context(source_module.retained_workspace_source(repository, manifest))
        build_environment = {**environment, "CARGO_TARGET_DIR": str(roots["bootstrap"]),
                             "CARGO": str(cargo), "RUSTC": str(rustc), "NORITO_SKIP_BINDINGS_SYNC": "1"}
        with (evidence / "publisher-metadata.json").open("xb") as output:
            metadata = subprocess.run([str(cargo), "metadata", "--locked", "--offline", "--no-deps",
                                       "--format-version", "1"], cwd=repository, env=build_environment,
                                      stdout=output, check=False)
        source.revalidate()
        metadata.check_returncode()
        revalidate_cargo()
        revalidate_rustc()
        package_id = publisher_package((evidence / "publisher-metadata.json").read_bytes(), repository)
        print("Building the reviewed normal publisher under retained source custody", flush=True)
        with (evidence / "publisher-build.jsonl").open("xb") as output:
            built = subprocess.run([str(cargo), "build", "--locked", "--offline", "--target", native,
                                    "-p", "iroha_release_bundle", "--no-default-features",
                                    "--features", "dev-tools", "--bin",
                                    "iroha_release_bundle", "--message-format=json-render-diagnostics"],
                                   cwd=repository, env=build_environment, stdout=output, check=False)
        source.revalidate()
        built.check_returncode()
        revalidate_cargo()
        revalidate_rustc()
        publisher = publisher_from_events((evidence / "publisher-build.jsonl").read_bytes(), repository,
                                          roots["bootstrap"], native, package_id)
        name = "invocation.ci0001"
        expected_bridge = roots["artifacts"] / "sumeragi-release" / manifest / "bridges" / name
        with retained_executable(publisher) as (descriptor, revalidate_publisher):
            print("Publishing the native observer through the maintained producer", flush=True)
            with (evidence / "bridge-receipt.tsv").open("xb") as output:
                published = subprocess.run([
                    f"/proc/self/fd/{descriptor}", "build-bridge-publish",
                    "--repo-root", str(repository), "--source-sha256", manifest,
                    "--cargo-target-dir", str(roots["cargo"]), "--artifact-root", str(roots["artifacts"]),
                    "--cargo-home", str(args.cargo_home), "--cargo-bin", str(cargo), "--rustc-bin", str(rustc),
                    "--cancel-request", str(args.run_root / "cancel-request.json"), "--name", name,
                ], cwd=repository, env=environment, pass_fds=(descriptor,), stdout=output, check=False)
            source.revalidate()
            published.check_returncode()
            revalidate_publisher()
            revalidate_cargo()
            revalidate_rustc()
        pins = bridge_receipt((evidence / "bridge-receipt.tsv").read_bytes(), expected_bridge)
        print("Running every harness test against the independent original publisher receipt", flush=True)
        result = subprocess.run([
            sys.executable, "-I", "-B", str(repository / "scripts/tests/run_zk_resource_harness_native.py"),
            "--source-root", str(repository), "--bridge-root", pins["bridge_dir"],
            "--module-sha256", pins["module_sha256"], "--provenance-sha256", pins["provenance_sha256"],
            "--source-manifest-sha256", manifest, "--native-target", native, "--all-tests",
        ], cwd=repository, env=environment, check=False)
        source.revalidate()
        result.check_returncode()
    (evidence / "completed.json").write_text(json.dumps({
        "scope": "native-harness-component-ci", "source_commit": args.source_commit,
        "source_manifest_sha256": manifest, "native_target": native, **pins,
    }, sort_keys=True) + "\n")


def main() -> None:
    """Require explicit CI source, tool, cache and fresh output roots."""
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    for name in ("source-root", "cargo-bin", "rustc-bin", "cargo-home", "run-root"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    run(parser.parse_args())


if __name__ == "__main__":
    main()
