#!/usr/bin/env python3
"""Build only the two unqualified Taira carriers on a native Linux ARM host.

Python 3.11+, a clean signed optimizations checkout, a public-only signer export,
explicit native-tool/environment SHA256 pins and an existing warm authenticated
Cargo lane are required in an owner-only JSON plan. No ambient compiler flags,
credentials, signing, deployment, tests or release qualification are accepted.
`plan-only` validates the closed plan; `build` retains original child stdout and
stderr and actual exit status. A fresh output is required for every attempt;
failed logs and the warm target are preserved. No child is signalled on interrupt.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import pwd
import re
import shutil
import stat
import struct
import subprocess
import sys
import time
import tomllib

HERE = Path(__file__).resolve().parent
LOADED_SOURCE = {"taira_native_carrier_build.py": Path(__file__).read_bytes()}


def source_module(name):
    """Execute maintained source bytes without consulting cached bytecode."""
    path = HERE / (name + ".py")
    raw = path.read_bytes()
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    LOADED_SOURCE[path.name] = raw
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


contract = source_module("release_artifact_contract")
cache = source_module("taira_cargo_cache")
artifact = source_module("taira_cargo_artifact")
release = source_module("taira_release")
PLAN_SCHEMA = "taira.native-carrier-build.plan.v1"
RESULT_SCHEMA = "taira.native-carrier-build.result.v1"
TARGET = "aarch64-unknown-linux-gnu"
BINARIES = (("iroha3d_taira", "irohad"), ("iroha", "iroha_cli"))
TOOL_NAMES = {"git", "gpg", "python", "cargo", "rustc", "rustdoc", "compiler", "linker"}
ENV_NAMES = {"PATH", "HOME", "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO", "RUSTC", "RUSTDOC",
             "RUSTUP_TOOLCHAIN", "CARGO_BUILD_JOBS", "CARGO_INCREMENTAL", "CARGO_NET_OFFLINE",
             "CARGO_ENCODED_RUSTFLAGS", "CARGO_PROFILE_DEV_SPLIT_DEBUGINFO",
             "CARGO_PROFILE_TEST_SPLIT_DEBUGINFO", "IROHA_GIT_COMMIT_HASH", "VERGEN_GIT_SHA",
             "CARGO_ZIGBUILD_PYTHON_PATH", "CARGO_ZIGBUILD_ZIG_PATH", "CC_ENABLE_DEBUG_OUTPUT",
             "LC_ALL", "PYTHONDONTWRITEBYTECODE", "PYTHONNOUSERSITE"}
MAX_PLAN = 64 * 1024
MAX_BINARY = 4 * 1024**3


def need(value, message):
    """Fail before publishing an artifact receipt when an invariant is unproved."""
    if not value:
        raise RuntimeError(message)


def canonical(value):
    """Use the shared byte-exact metadata encoding."""
    return contract.canonical_json_bytes(value)


def sha(raw):
    """Hash public source or observation bytes."""
    return hashlib.sha256(raw).hexdigest()


def exact(value, keys, label):
    """Reject unknown fields, including extra qualification or secret inputs."""
    need(isinstance(value, dict) and set(value) == set(keys), label + " fields differ")


def absolute(raw):
    """Admit normalized absolute path text without following filesystem paths."""
    need(isinstance(raw, str) and raw and "\0" not in raw and "\n" not in raw,
         "invalid public path")
    path = Path(raw)
    need(path.is_absolute() and str(path) == os.path.abspath(path), "path must be absolute and normalized")
    return path


def fingerprint(raw, length=64):
    """Validate exact public digest or Git object notation."""
    need(isinstance(raw, str) and re.fullmatch(r"[0-9a-f]{" + str(length) + "}", raw),
         "invalid public digest")


def validate_plan(plan):
    """Validate the dev-only closed contract; never infer pins from running tools."""
    exact(plan, {"schema", "source", "target_dir", "lane_owner_repo_root", "output_dir",
                 "tools", "environment", "capacity"}, "plan")
    need(plan["schema"] == PLAN_SCHEMA, "unsupported native carrier plan")
    exact(plan["source"], {"repo_root", "commit", "tree", "signer", "cargo_lock_sha256", "public_key"}, "source")
    source = plan["source"]
    for key in ("commit", "tree"):
        fingerprint(source[key], 40)
    fingerprint(source["cargo_lock_sha256"])
    need(isinstance(source["signer"], str) and re.fullmatch(r"(?:[0-9A-F]{40}|[0-9A-F]{64})", source["signer"]),
         "source signer must be a full OpenPGP signing-key fingerprint")
    exact(source["public_key"], {"path", "sha256", "size"}, "public key")
    fingerprint(source["public_key"]["sha256"])
    need(type(source["public_key"]["size"]) is int and 0 < source["public_key"]["size"] <= MAX_PLAN,
         "public key export exceeds bound")
    absolute(source["public_key"]["path"])
    root = absolute(source["repo_root"])
    target = absolute(plan["target_dir"])
    output = absolute(plan["output_dir"])
    absolute(plan["lane_owner_repo_root"])
    need(not output.is_relative_to(root) and not root.is_relative_to(output)
         and not output.is_relative_to(target) and not target.is_relative_to(output),
         "output must be separate from source and warm target")
    exact(plan["tools"], TOOL_NAMES, "tools")
    for name, row in plan["tools"].items():
        exact(row, {"invocation", "path", "sha256", "size"}, "tool " + name)
        absolute(row["invocation"]); absolute(row["path"])
        fingerprint(row["sha256"])
        need(type(row["size"]) is int and 0 < row["size"] <= MAX_BINARY, "invalid native tool size")
    exact(plan["environment"], ENV_NAMES, "environment")
    env = plan["environment"]
    need(all(isinstance(value, str) and value and "\0" not in value and "\n" not in value
             for value in env.values()), "environment values must be nonempty public text")
    for key in ("PATH",):
        for component in env[key].split(":"):
            absolute(component)
    for key in ("HOME", "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_ZIGBUILD_ZIG_PATH"):
        absolute(env[key])
    expected = {"CARGO_TARGET_DIR": str(target), "CARGO_BUILD_JOBS": "6", "CARGO_INCREMENTAL": "1",
                "CARGO_NET_OFFLINE": "true", "CARGO_PROFILE_DEV_SPLIT_DEBUGINFO": "unpacked",
                "CARGO_PROFILE_TEST_SPLIT_DEBUGINFO": "unpacked", "LC_ALL": "C",
                "PYTHONDONTWRITEBYTECODE": "1", "PYTHONNOUSERSITE": "1",
                "IROHA_GIT_COMMIT_HASH": source["commit"], "VERGEN_GIT_SHA": source["commit"],
                "CARGO_ZIGBUILD_PYTHON_PATH": "/usr/bin/false", "CC_ENABLE_DEBUG_OUTPUT": "1"}
    expected.update({name.upper(): plan["tools"][name]["path"] for name in ("cargo", "rustc", "rustdoc")})
    expected["CARGO_ENCODED_RUSTFLAGS"] = ("-Clinker=" + plan["tools"]["compiler"]["path"]
                                          + "\x1f-Clink-arg=-fuse-ld=" + plan["tools"]["linker"]["invocation"])
    need(all(env[key] == value for key, value in expected.items()), "native build environment differs from fixed dev policy")
    need(re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", env["RUSTUP_TOOLCHAIN"]), "invalid pinned Rust channel")
    exact(plan["capacity"], {"cargo_additional_bytes", "capture_additional_bytes"}, "capacity")
    need(all(type(value) is int and 0 <= value <= 1024**4 for value in plan["capacity"].values()),
         "invalid owner-selected additional capacity")
    return plan


def read_plan(path):
    """Read one bounded, owner-held plan and reject duplicate or noncanonical JSON."""
    path = release.real_path(absolute(str(path)))
    info = path.lstat()
    need(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) in (0o400, 0o600),
         "plan must remain owner-only")
    stable, raw = contract.stable_read_path(path, max_size=MAX_PLAN)
    def pairs(rows):
        value = {}
        for key, item in rows:
            need(key not in value, "duplicate plan field")
            value[key] = item
        return value
    plan = json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: need(False, "nonfinite plan value"))
    validate_plan(plan)
    need(raw == canonical(plan), "plan must use exact canonical bytes")
    return plan, raw, stable


def private_directory(path):
    """Admit an existing direct owner-private path without repairing permissions."""
    release.real_path(path)
    info = path.lstat()
    need(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700,
         "directory must remain existing owner-held 0700: " + str(path))


def verify_tools(plan):
    """Authenticate every invoked tool, its canonical resolution and metadata."""
    rows = {}
    for name, row in plan["tools"].items():
        path, invocation = absolute(row["path"]), absolute(row["invocation"])
        release.real_path(path)
        need(invocation.resolve(strict=True) == path, "tool invocation resolution differs: " + name)
        info = path.lstat()
        need(info.st_uid in (0, os.geteuid()) and not info.st_mode & 0o022 and info.st_mode & stat.S_IXUSR,
             "native tool custody differs: " + name)
        actual = contract.stable_hash_path(path, max_size=MAX_BINARY)
        need(actual.sha256 == row["sha256"] and actual.size == row["size"], "native tool bytes differ: " + name)
        need(invocation.resolve(strict=True) == path, "tool invocation changed while hashing")
        rows[name] = dict(row, identity=list(release.file_identity(path.lstat())))
    need(Path(sys.executable).resolve() == absolute(plan["tools"]["python"]["path"]),
         "controller interpreter differs from pinned Python")
    need(shutil.which("git", path=plan["environment"]["PATH"]) == plan["tools"]["git"]["invocation"],
         "maintained capture PATH must select the pinned Git invocation")
    return rows


def run_probe(argv, env, output, label, *, max_output=1024**2, lock_fds=()):
    """Retain exact native probe output before interpreting its exit or metadata."""
    release.write_record(output / (label + ".request.json"), {"argv": argv, "cwd": "/", "environment": env})
    exit_code = run_build(argv, env, output, lock_fds, label=label)
    release.freeze(output / (label + ".stdout")); release.freeze(output / (label + ".stderr"))
    release.write_record(output / (label + ".exit.json"), {"exit_code": exit_code})
    with (output / (label + ".stdout")).open("rb") as stream:
        stdout = stream.read(max_output + 1)
    with (output / (label + ".stderr")).open("rb") as stream:
        stderr = stream.read(max_output + 1)
    need(len(stdout) <= max_output and len(stderr) <= max_output, "native probe output exceeds bound")
    result = subprocess.CompletedProcess(argv, exit_code, stdout, stderr)
    need(result.returncode == 0, "native " + label + " failed; original output retained")
    return result


def public_keyring(plan, output, env):
    """Delegate public-only key classification and Git signature verification to GPG."""
    row = plan["source"]["public_key"]
    path = release.real_path(absolute(row["path"]))
    info = path.lstat()
    need(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) in (0o400, 0o600)
         and info.st_nlink == 1 and info.st_size == row["size"], "public key export custody differs")
    home = contract.create_fresh_directory(output / "source-public-keyring", mode=0o700)
    gpg = plan["tools"]["gpg"]["path"]
    command = [gpg, "--batch", "--no-options", "--homedir", str(home), "--no-auto-key-retrieve",
               "--auto-key-locate", "clear"]
    shown = run_probe(command + ["--with-colons", "--import-options", "show-only", "--dry-run", "--import", str(path)],
                      env, output, "source-key-classification")
    records = [line.split(b":") for line in shown.stdout.splitlines()]
    need(any(row[0] == b"pub" for row in records) and not any(row[0] in (b"sec", b"ssb") for row in records),
         "source key input must be public-only")
    actual, raw = contract.stable_read_path(path, max_size=MAX_PLAN)
    need(actual.sha256 == row["sha256"] and actual.size == row["size"], "public key export differs from plan")
    contract.exclusive_write_bytes(output / "source-signer-public-key.gpg", raw, mode=0o600)
    release.freeze(output / "source-signer-public-key.gpg")
    run_probe(command + ["--import", str(output / "source-signer-public-key.gpg")], env, output, "source-key-import")
    # Git supplies its own verification arguments. Only this empty, owner-held
    # public keyring config is visible; network key retrieval remains disabled.
    contract.exclusive_write_bytes(home / "gpg.conf", b"no-auto-key-retrieve\nauto-key-locate clear\n", mode=0o600)
    return env | {"GNUPGHOME": str(home)}


def git_runner(plan, env):
    """Construct pinned native Git calls, excluding replace objects and local hooks."""
    def git(root, *args):
        command = [plan["tools"]["git"]["path"], "--no-replace-objects", "-c", "core.hooksPath=/dev/null",
                   "-c", "gpg.format=openpgp", "-c", "gpg.program=" + plan["tools"]["gpg"]["path"], *args]
        result = subprocess.run(command, cwd=root, env=env, stdin=subprocess.DEVNULL,
                                capture_output=True, check=False, timeout=60)
        need(result.returncode == 0, "native Git " + args[0] + " failed")
        return result.stdout.strip()
    return git


def verify_source(plan, git, env, output, label):
    """Prove the exact clean signed HEAD, tree, Cargo.lock and executed source closure."""
    source = plan["source"]
    root, commit = absolute(source["repo_root"]), source["commit"]
    private_directory(root)
    need(git(root, "rev-parse", "--show-toplevel") == os.fsencode(root), "selected Git root differs")
    need(git(root, "branch", "--show-current") == b"optimizations", "native carrier build requires optimizations")
    need(git(root, "rev-parse", "HEAD").decode() == commit, "selected source HEAD differs")
    need(git(root, "rev-parse", commit + "^{tree}").decode() == source["tree"], "selected source tree differs")
    need(git(root, "write-tree").decode() == source["tree"], "selected source index differs")
    need(not git(root, "status", "--porcelain=v1", "--untracked-files=normal"), "selected source checkout is not clean")
    command = [plan["tools"]["git"]["path"], "--no-replace-objects", "-c", "gpg.format=openpgp",
               "-c", "gpg.program=" + plan["tools"]["gpg"]["path"], "-C", str(root), "verify-commit", "--raw", commit]
    verified = run_probe(command, env, output, label + "-signature")
    signatures = [line.split()[2].decode() for line in verified.stderr.splitlines()
                  if line.startswith(b"[GNUPG:] VALIDSIG ")]
    need(signatures == [source["signer"]]
         and git(root, "show", "--no-patch", "--format=%GF", commit).decode() == source["signer"],
         "source signature does not match owner-pinned full signer")
    entries = release.commit_entries(root, commit)
    rows = release.source_snapshot(root, entries)
    by_path = {row["path"]: row for row in rows}
    need(by_path["Cargo.lock"]["sha256"] == source["cargo_lock_sha256"], "Cargo.lock differs from plan")
    need(Path(__file__).resolve() == root / "scripts/taira_native_carrier_build.py",
         "controller must execute from the selected signed checkout")
    for name, raw in LOADED_SOURCE.items():
        expected = by_path.get("scripts/" + name)
        need(expected is not None and expected.get("kind") == "regular" and expected["sha256"] == sha(raw)
             and (root / "scripts" / name).read_bytes() == raw, "executed controller source differs from signed objects")
    return {"commit": commit, "tree": source["tree"], "signer": source["signer"],
            "cargo_lock_sha256": source["cargo_lock_sha256"], "checkout_snapshot_sha256": sha(canonical(rows)),
            "controller_sha256": {name: sha(raw) for name, raw in LOADED_SOURCE.items()}}, entries


def check_cargo_home(plan):
    """Admit cache bytes while refusing Cargo configuration or credential inputs."""
    home = absolute(plan["environment"]["CARGO_HOME"])
    private_directory(home)
    allowed = {"registry", "git", ".package-cache", ".package-cache-mutate", ".global-cache",
               ".global-cache-shm", ".global-cache-wal"}
    for path in home.iterdir():
        need(path.name in allowed, "unexpected isolated Cargo home input")
        if path.name in ("registry", "git"):
            need(path.is_dir() and path.resolve(strict=True).stat().st_uid == os.geteuid(), "Cargo cache custody differs")
        else:
            info = path.lstat()
            need(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid() and info.st_nlink == 1
                 and not info.st_mode & 0o022, "Cargo cache lock custody differs")
    need(not any(os.path.lexists(path) for path in ("/.cargo/config", "/.cargo/config.toml")),
         "root Cargo configuration prevents immutable build isolation")


def build_command(source, target, cargo):
    """Select exactly the two native dev carriers and retain Cargo's JSON emission."""
    command = [cargo, "build", "--config", str(source / ".cargo/config.toml"),
               "--manifest-path", str(source / "Cargo.toml"), "--target-dir", str(target),
               "--locked", "--offline", "--message-format=json-render-diagnostics"]
    for name, package in BINARIES:
        command.extend(("-p", package, "--bin", name))
    return command


def metadata_packages(source, env, output, lock_fds=()):
    """Retain the actual offline package preflight streams and select local owners."""
    command = [env["CARGO"], "--config", str(source / ".cargo/config.toml"), "metadata",
               "--manifest-path", str(source / "Cargo.toml"), "--locked", "--offline", "--format-version=1"]
    result = run_probe(command, env, output, "cargo-metadata", max_output=64 * 1024**2, lock_fds=lock_fds)
    value = json.loads(result.stdout)
    need(isinstance(value, dict) and isinstance(value.get("packages"), list), "invalid native Cargo package closure")
    packages = {row["name"] for row in value["packages"] if row["source"] is None}
    need(all(isinstance(name, str) and name for name in packages) and {"irohad", "iroha_cli"} <= packages,
         "Cargo closure lacks selected carrier owners")
    return packages


def run_build(command, env, output, lock_fds, *, label="build"):
    """Observe a real child with separate original streams and inherited lane locks."""
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
    stdout = os.open(output / (label + ".stdout"), flags, 0o600)
    try:
        stderr = os.open(output / (label + ".stderr"), flags, 0o600)
        try:
            child = subprocess.Popen(command, cwd="/", env=env, stdin=subprocess.DEVNULL,
                                     stdout=stdout, stderr=stderr, pass_fds=tuple(lock_fds),
                                     start_new_session=True, umask=0o077)
            started_path = "started.json" if label == "build" else label + ".started.json"
            release.write_record(output / started_path, {"pid": child.pid, "started_ns": time.time_ns(),
                                                        "cwd": "/", "argv": command})
            started = time.monotonic()
            while True:
                try:
                    result = child.wait(timeout=30)
                    break
                except subprocess.TimeoutExpired:
                    print(f"[taira-native-carrier] {label} running ({time.monotonic() - started:.0f}s); original logs: {output}", flush=True)
            os.fsync(stdout); os.fsync(stderr)
            return result
        finally:
            os.close(stderr)
    finally:
        os.close(stdout)
    # No context-manager Popen wait/termination: an interrupted observer leaves
    # the Cargo child, original streams and inherited flocks alive.


def carrier_owner_graph(metadata_log, source, target, frozen):
    """Bind retained native Cargo owners to manifests and bodies in signed source."""
    info, raw = contract.stable_read_path(metadata_log, max_size=64 * 1024**2)
    value = json.loads(raw)
    need(isinstance(value, dict) and value.get("version") == 1
         and value.get("workspace_root") == str(source) and value.get("target_directory") == str(target),
         "Cargo owner graph workspace or target differs")
    packages, members = value.get("packages"), value.get("workspace_members")
    need(isinstance(packages, list) and all(isinstance(row, dict) for row in packages)
         and isinstance(members, list) and all(isinstance(member, str) for member in members)
         and len(members) == len(set(members)), "Cargo owner graph package membership is invalid")
    signed = {row["path"]: row for row in frozen}
    need(len(signed) == len(frozen), "signed source contains duplicate paths")

    def body(path, *, manifest=False):
        need(path.is_relative_to(source), "Cargo owner graph path is outside signed source")
        row = signed.get(path.relative_to(source).as_posix())
        need(isinstance(row, dict) and row.get("kind") == "regular"
             and row.get("index_mode") in ("100644", "100755"),
             "Cargo owner graph path is not a signed regular source file")
        if manifest:
            actual, payload = contract.stable_read_path(path, max_size=1024**2)
        else:
            actual, payload = contract.stable_hash_path(path, max_size=64 * 1024**2), None
        need(actual.sha256 == row["sha256"] and actual.size == row["size"],
             "Cargo owner graph signed source bytes differ")
        return actual.sha256, payload

    root_sha, root_raw = body(source / "Cargo.toml", manifest=True)
    workspace = tomllib.loads(root_raw.decode("utf-8")).get("workspace", {}).get("package", {})
    owners = {}
    for name, package in BINARIES:
        selected = [row for row in packages if row.get("name") == package and row.get("source") is None]
        need(len(selected) == 1, "Cargo owner graph must select exactly one local carrier package")
        row = selected[0]
        package_id = row.get("id")
        need(isinstance(package_id, str) and members.count(package_id) == 1
             and sum(candidate.get("id") == package_id for candidate in packages) == 1,
             "Cargo owner graph selected package ID is not a unique workspace member")
        manifest = absolute(row.get("manifest_path"))
        need(manifest.name == "Cargo.toml", "Cargo owner graph manifest is not Cargo.toml")
        manifest_sha, manifest_raw = body(manifest, manifest=True)
        declared = tomllib.loads(manifest_raw.decode("utf-8"))
        local = declared.get("package", {})
        need(local.get("name") == package and "workspace" not in local,
             "Cargo owner graph signed package owner differs")

        def package_value(key):
            setting = local.get(key)
            if setting == {"workspace": True}:
                setting = workspace.get(key)
            need(isinstance(setting, str) and setting, "Cargo owner graph signed package " + key + " is invalid")
            return setting

        version, edition = package_value("version"), package_value("edition")
        # Cargo's local package ID omits the package name when the directory
        # already has that name; a bins owner has the explicit name@version.
        fragment = version if manifest.parent.name == package else package + "@" + version
        need(row.get("version") == version
             and package_id == "path+" + manifest.parent.as_uri() + "#" + fragment,
             "Cargo owner graph package ID or version differs from signed manifest")
        targets = row.get("targets")
        need(isinstance(targets, list) and all(isinstance(item, dict) for item in targets),
             "Cargo owner graph target list is invalid")
        selected = [item for item in targets if item.get("name") == name and item.get("kind") == ["bin"]]
        bins = declared.get("bin", [])
        need(isinstance(bins, list) and all(isinstance(item, dict) for item in bins),
             "Cargo owner graph signed bin declarations are invalid")
        declarations = [item for item in bins if item.get("name") == name]
        need(len(selected) == len(declarations) == 1,
             "Cargo owner graph must bind one explicit signed bin declaration")
        native, declaration = selected[0], declarations[0]
        relative = declaration.get("path")
        need(isinstance(relative, str) and relative and not Path(relative).is_absolute(),
             "Cargo owner graph signed bin path is invalid")
        src = absolute(os.path.abspath(manifest.parent / relative))
        src_sha, _ = body(src)
        identity = {"name": name, "kind": ["bin"], "crate_types": ["bin"], "src_path": str(src),
                    "edition": declaration.get("edition", edition)}
        features = declaration.get("required-features", [])
        need(isinstance(features, list) and all(isinstance(feature, str) for feature in features)
             and all(native.get(key) == expected for key, expected in identity.items())
             and native.get("required-features", []) == features,
             "Cargo owner graph target differs from signed bin declaration")
        owners[name] = {"package": package, "package_id": package_id, "manifest_path": str(manifest),
                        "manifest_sha256": manifest_sha, "src_sha256": src_sha,
                        "target": identity, "required_features": features}
    return {"schema": "taira.native-carrier-build.cargo-owners.v1", "metadata_sha256": info.sha256,
            "metadata_size": info.size, "workspace_manifest_sha256": root_sha, "owners": owners}


def artifact_emissions(log, source, target, owner_graph=None):
    """Bind each captured executable to this successful Cargo compiler emission."""
    wanted = {name: package for name, package in BINARIES}
    need(isinstance(owner_graph, dict) and owner_graph.get("schema") == "taira.native-carrier-build.cargo-owners.v1"
         and isinstance(owner_graph.get("owners"), dict) and set(owner_graph["owners"]) == set(wanted),
         "source-bound native Cargo owner graph is required")
    found, finished = {}, []
    with log.open("rb") as stream:
        while raw := stream.readline(8 * 1024**2 + 1):
            need(len(raw) <= 8 * 1024**2, "Cargo JSON line exceeds observation bound")
            try:
                value = json.loads(raw)
            except ValueError:
                continue
            if not isinstance(value, dict):
                continue
            if value.get("reason") == "build-finished":
                finished.append(value.get("success"))
            # The iroha library and CLI share a target name. Select executable
            # emissions before applying the expected carrier names and owners.
            native = value.get("target")
            if value.get("reason") != "compiler-artifact" or not isinstance(native, dict) or native.get("kind") != ["bin"]:
                continue
            name = native.get("name")
            if name not in wanted:
                continue
            owner = owner_graph["owners"][name]
            executable = str(target / "debug" / name)
            need(name not in found and owner.get("package") == wanted[name]
                 and value.get("package_id") == owner.get("package_id")
                 and value.get("manifest_path") == owner.get("manifest_path")
                 and all(native.get(key) == expected for key, expected in owner["target"].items())
                 and native.get("required-features", []) == owner["required_features"]
                 and isinstance(value.get("profile"), dict) and value["profile"].get("test") is False
                 and value.get("executable") == executable and isinstance(value.get("filenames"), list)
                 and executable in value["filenames"],
                 "Cargo carrier emission differs from selected signed source")
            found[name] = value
    need(finished == [True] and set(found) == set(wanted), "Cargo did not emit both successful dev carriers")
    return found


def elf_header(header):
    """Require an ordinary 64-bit little-endian AArch64 ELF executable."""
    need(len(header) >= 64 and header[:7] == b"\x7fELF\x02\x01\x01"
         and struct.unpack_from("<HHI", header, 16)[0] in (2, 3)
         and struct.unpack_from("<HHI", header, 16)[1:] == (183, 1),
         "carrier is not a native AArch64 ELF executable")


def capture_artifacts(target, output):
    """Copy verified native outputs without changing warm Cargo aliases."""
    originals = [(name, package, artifact.cargo_hash_path(target / "debug" / name, max_size=MAX_BINARY))
                 for name, package in BINARIES]
    release.capacity_preflight([(output, sum(info.size for _, _, info in originals), "actual two-carrier capture")])
    destination = contract.create_fresh_directory(output / "bin", mode=0o700)
    rows = []
    for name, package, expected in originals:
        need(expected.size >= 64 and expected.mode & stat.S_IXUSR, "Cargo carrier must be executable")
        path = destination / name
        with artifact.cargo_open_relative(target, "debug/" + name, expected=expected) as source:
            elf_header(os.read(source, 64)); os.lseek(source, 0, os.SEEK_SET)
            with contract.exclusive_output_fd(path, mode=0o755) as fd:
                digest, size = hashlib.sha256(), 0
                while block := os.read(source, 1024 * 1024):
                    digest.update(block); size += len(block)
                    view = memoryview(block)
                    while view:
                        written = os.write(fd, view)
                        need(written > 0, "carrier capture made no progress")
                        view = view[written:]
                need(size == expected.size and digest.hexdigest() == expected.sha256, "Cargo carrier changed during capture")
        release.freeze(path)
        actual = contract.stable_hash_path(path, max_size=MAX_BINARY)
        need(actual.sha256 == expected.sha256 and actual.size == expected.size and actual.mode == 0o500,
             "retained carrier differs from actual build")
        rows.append({"name": name, "package": package, "path": str(path), "sha256": actual.sha256, "size": actual.size})
    release.freeze(destination, directory=True)
    return rows


def execute(plan, raw_plan):
    """Build under maintained locks; publish artifacts only after final invariants."""
    need(sys.platform == "linux" and platform.machine() in ("aarch64", "arm64"), "native Linux ARM host is required")
    root, target, output = (absolute(plan["source"]["repo_root"]), absolute(plan["target_dir"]), absolute(plan["output_dir"]))
    lane_owner = absolute(plan["lane_owner_repo_root"])
    for path in (root, target, lane_owner, output.parent):
        private_directory(path)
    need(not os.path.lexists(output), "output already exists; preserve it and select a fresh attempt")
    need(plan["environment"]["HOME"] == pwd.getpwuid(os.geteuid()).pw_dir, "HOME differs from the native build owner")
    with release.cargo_lane(lane_owner, target, "release") as mode_lock:
        with release.source_lane(root, target) as (source, source_lock):
            output = contract.create_fresh_directory(output, mode=0o700)
            with release.preparation_lock(output, purpose="native dev carrier attempt") as attempt_lock:
                exit_code, stage = None, "admission"
                base = {"schema": RESULT_SCHEMA, "profile": "dev", "target": TARGET, "checks_run": False,
                        "native_release_qualified": False, "application_ready": False, "deployed": False,
                        "jobs": 6, "warm_target_reused": True, "repo_root": str(root), "source_root": str(source),
                        "target_dir": str(target), "lane_owner_repo_root": str(lane_owner),
                        "plan_sha256": sha(raw_plan), "cwd": "/"}
                try:
                    contract.exclusive_write_bytes(output / "plan.json", raw_plan, mode=0o600)
                    release.freeze(output / "plan.json")
                    tools_before = verify_tools(plan)
                    env = dict(plan["environment"])
                    check_cargo_home(plan)
                    key_env = public_keyring(plan, output, env)
                    git = git_runner(plan, key_env)
                    # Maintained capture uses this pinned executable environment;
                    # no cached or primary-worktree helper is imported.
                    release.git = git
                    release.child_environment = lambda inherited, selected: dict(key_env)
                    before, entries = verify_source(plan, git, key_env, output, "before")
                    rust = run_probe([env["RUSTC"], "--version", "--verbose"], env, output, "rustc-version").stdout.decode()
                    need("host: " + TARGET in rust.splitlines(), "Rust toolchain host is not native Linux ARM")
                    need("release: " + env["RUSTUP_TOOLCHAIN"] in rust.splitlines(), "Rust toolchain release differs from plan")
                    capacity = release.capacity_preflight([
                        (target, release.signed_source_size(root, plan["source"]["commit"], entries), "signed source capture"),
                        (target, plan["capacity"]["cargo_additional_bytes"], "owner-selected Cargo additional bytes"),
                        (output, plan["capacity"]["capture_additional_bytes"], "owner-selected output additional bytes")])
                    stage = "source-capture"
                    source = release.capture_source(root, source, target, plan["source"]["commit"], entries)
                    frozen = release.frozen_snapshot(source, entries, target)
                    need(tomllib.loads((source / "rust-toolchain.toml").read_text())["toolchain"]["channel"] == env["RUSTUP_TOOLCHAIN"],
                         "captured Rust channel differs from plan")
                    need(env["CARGO_ZIGBUILD_ZIG_PATH"] == str(source / "scripts/zig_linux_gnu.py"),
                         "captured wrapper environment path differs")
                    command = build_command(source, target, env["CARGO"])
                    base.update(argv=command, environment=env, tools=tools_before, source_before=before,
                                frozen_snapshot_sha256=sha(canonical(frozen)), capacity_observation=capacity,
                                source_public_key={**plan["source"]["public_key"], "retained_path": str(output / "source-signer-public-key.gpg")})
                    release.write_record(output / "request.json", base)
                    stage = "fingerprint-admission"
                    packages = metadata_packages(source, env, output, (mode_lock, source_lock, attempt_lock))
                    owner_graph = carrier_owner_graph(output / "cargo-metadata.stdout", source, target, frozen)
                    release.write_record(output / "cargo-owner-graph.json", owner_graph)
                    cache.admit_source_fingerprints(source, target, TARGET, packages)
                    need(verify_tools(plan) == tools_before, "native tools changed before Cargo")
                    need(release.frozen_snapshot(source, entries, target) == frozen, "captured source changed before Cargo")
                    stage = "cargo"
                    exit_code = run_build(command, env, output, (mode_lock, source_lock, attempt_lock))
                    release.freeze(output / "build.stdout"); release.freeze(output / "build.stderr")
                    release.write_record(output / "cargo-exit.json", {"exit_code": exit_code, "finished_ns": time.time_ns()})
                    need(exit_code == 0, "native Cargo build failed; original stdout/stderr retained")
                    stage = "postcheck"
                    with cache.source_fingerprints(source, target, TARGET, packages, repair=False):
                        after, after_entries = verify_source(plan, git, key_env, output, "after")
                        need(after == before and after_entries == entries, "selected source changed during build")
                        need(verify_tools(plan) == tools_before, "native tools changed during build")
                        check_cargo_home(plan)
                        need(release.frozen_snapshot(source, entries, target) == frozen, "immutable build source changed during Cargo")
                        emissions = artifact_emissions(output / "build.stdout", source, target, owner_graph)
                        stage = "artifact-capture"
                        rows = capture_artifacts(target, output)
                        need(release.frozen_snapshot(source, entries, target) == frozen, "immutable source changed during capture")
                        need(verify_tools(plan) == tools_before, "native tools changed during capture")
                        final, final_entries = verify_source(plan, git, key_env, output, "final")
                        need(final == before and final_entries == entries, "selected source changed during capture")
                    result = {**base, "exit_code": exit_code, "source_after": after, "source_final": final,
                              "source_unchanged": True, "toolchain_unchanged": True, "artifacts": rows,
                              "cargo_owner_graph_sha256": sha(canonical(owner_graph)),
                              "cargo_emissions": emissions, "finished_ns": time.time_ns()}
                    release.write_record(output / "manifest.json", result)
                    release.freeze(output, directory=True)
                    return result
                except Exception as error:
                    # Failure evidence never contains artifact rows or a success
                    # manifest. Original logs and any partial capture stay put.
                    release.write_record(output / "failure.json", {**base, "stage": stage, "exit_code": exit_code,
                                                                  "error_type": type(error).__name__,
                                                                  "finished_ns": time.time_ns()})
                    raise


def main():
    """Expose explicit validation and execution without source mutation or signing."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("plan-only", "build"))
    parser.add_argument("--plan", type=Path, required=True, help="canonical owner-only public build plan")
    args = parser.parse_args()
    try:
        plan, raw, _ = read_plan(args.plan)
        if args.action == "plan-only":
            sys.stdout.buffer.write(canonical(plan))
        else:
            result = execute(plan, raw)
            print("[taira-native-carrier] retained dev carriers: " + str(Path(plan["output_dir"]) / "manifest.json"))
            need(result["exit_code"] == 0, "carrier build was unsuccessful")
    except (RuntimeError, OSError, ValueError) as error:
        print("[taira-native-carrier] failed: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
