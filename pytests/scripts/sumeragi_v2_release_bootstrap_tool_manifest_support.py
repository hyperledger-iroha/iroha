"""Shared host-tool support for Sumeragi bootstrap tests."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import sysconfig


RELEASE_SHELL_UTILITY_NAMES = (
    "awk", "basename", "cat", "chmod", "cmp", "cp", "cut", "diff",
    "dirname", "env", "find", "grep", "ln", "ls", "mkdir", "mkfifo",
    "mktemp", "mv", "openssl", "rm", "rmdir", "sed", "sh", "sleep",
    "tail", "tee", "tr", "uname", "wc", "xargs",
    "shasum" if sys.platform == "darwin" else "sha256sum",
)
RELEASE_LANGUAGE_TOOL_NAMES = (
    "cargo", "cargo-verus", "git-index-pack", "git-upload-pack", "java",
    "node", "rustc", "swift", "tlapm", "verus",
)
REQUIRED_RUNNER_TOOL_NAMES = tuple(
    sorted((*RELEASE_SHELL_UTILITY_NAMES, *RELEASE_LANGUAGE_TOOL_NAMES))
)
_FRAMEWORK_STDLIB_OMITTED_PATHS = frozenset({"site-packages"})
PROBE_OPERATION_IDS = {
    "awk": "release-tool.awk-program.v1",
    "basename": "release-tool.basename-path.v1",
    "cargo": "release-tool.cargo-version.v1",
    "cargo-verus": "release-tool.cargo-verus-help.v1",
    "cat": "release-tool.cat-file.v1",
    "chmod": "release-tool.chmod-mode.v1",
    "cmp": "release-tool.cmp-different-quiet.v1",
    "cp": "release-tool.cp-file.v1",
    "cut": "release-tool.cut-byte.v1",
    "diff": "release-tool.diff-different-brief.v1",
    "dirname": "release-tool.dirname-path.v1",
    "env": "release-tool.env-closed.v1",
    "find": "release-tool.find-file.v1",
    "git-index-pack": "release-tool.git-index-pack-empty.v1",
    "git-upload-pack": "release-tool.git-upload-pack-missing.v1",
    "grep": "release-tool.grep-exact.v1",
    "java": "release-tool.java-version.v1",
    "ln": "release-tool.ln-hardlink.v1",
    "ls": "release-tool.ls-entry.v1",
    "mkdir": "release-tool.mkdir-directory.v1",
    "mkfifo": "release-tool.mkfifo-fifo.v1",
    "mktemp": "release-tool.mktemp-file.v1",
    "mv": "release-tool.mv-file.v1",
    "node": "release-tool.node-exec-path.v1",
    "openssl": "release-tool.openssl-sha256.v1",
    "rm": "release-tool.rm-file.v1",
    "rmdir": "release-tool.rmdir-directory.v1",
    "rustc": "release-tool.rustc-version.v1",
    "sed": "release-tool.sed-first-line.v1",
    "sh": "release-tool.sh-builtin-output.v1",
    ("shasum" if sys.platform == "darwin" else "sha256sum"): (
        "release-tool.shasum-empty.v1"
        if sys.platform == "darwin"
        else "release-tool.sha256sum-empty.v1"
    ),
    "sleep": "release-tool.sleep-duration.v1",
    "swift": "release-tool.swift-version.v1",
    "tail": "release-tool.tail-last-line.v1",
    "tee": "release-tool.tee-file.v1",
    "tlapm": "release-tool.tlapm-version.v1",
    "tr": "release-tool.tr-byte.v1",
    "uname": "release-tool.uname-system.v1",
    "verus": "release-tool.verus-version.v1",
    "wc": "release-tool.wc-empty.v1",
    "xargs": "release-tool.xargs-protected-shell.v1",
}


def fixture_tool_probe_helper() -> bytes:
    """Return a deterministic fixture producer for probe-plumbing tests.

    The production helper has its own adversarial suite. Bootstrap/receipt
    fixtures use this non-executing producer so those tests cannot launch
    Cargo, SDK engines, or host utilities merely to exercise provenance flow.
    """

    operations = repr(dict(sorted(PROBE_OPERATION_IDS.items())))
    names = repr(REQUIRED_RUNNER_TOOL_NAMES)
    return f'''from pathlib import Path
import argparse
import hashlib
import json
import os

OPERATIONS = {operations}
NAMES = {names}
parser = argparse.ArgumentParser()
parser.add_argument("--tool-manifest", type=Path, required=True)
parser.add_argument("--expected-tool-manifest-sha256", required=True)
parser.add_argument("--probe-root", type=Path, required=True)
args = parser.parse_args()
data = args.tool_manifest.read_bytes()
if hashlib.sha256(data).hexdigest() != args.expected_tool_manifest_sha256:
    raise SystemExit(2)
manifest = json.loads(data)
if set(manifest) != {{"schema_version", "tools"}} or manifest["schema_version"] != 1:
    raise SystemExit(2)
tools = manifest["tools"]
if tuple(sorted(tools)) != NAMES or len(tools) != 41:
    raise SystemExit(2)
if args.probe_root.exists() or args.probe_root.is_symlink():
    raise SystemExit(2)
digest = hashlib.sha256(b"fixture-probe-contract-v1").hexdigest()
records = {{}}
for name in NAMES:
    record = tools[name]
    if set(record) != {{"archive_id", "path", "sha256"}}:
        raise SystemExit(2)
    path = Path(record["path"])
    metadata = path.lstat()
    if hashlib.sha256(path.read_bytes()).hexdigest() != record["sha256"]:
        raise SystemExit(2)
    records[name] = {{
        "archive_id": record["archive_id"],
        "exit_status": 128 if name in {{"git-index-pack", "git-upload-pack"}} else 1 if name in {{"cmp", "diff"}} else 0,
        "invocation_sha256": hashlib.sha256(("invocation:" + OPERATIONS[name]).encode()).hexdigest(),
        "mode": "0500",
        "operation_id": OPERATIONS[name],
        "postcondition_sha256": hashlib.sha256(("postcondition:" + OPERATIONS[name]).encode()).hexdigest(),
        "sha256": record["sha256"],
        "size_bytes": metadata.st_size,
        "stderr_sha256": hashlib.sha256(b"").hexdigest(),
        "stderr_size_bytes": 0,
        "stdout_sha256": hashlib.sha256(b"").hexdigest(),
        "stdout_size_bytes": 0,
    }}
value = {{
    "format": "iroha-sumeragi-v2-release-tool-functional-probes",
    "host_family": "darwin" if os.uname().sysname == "Darwin" else "linux",
    "probe_contract_sha256": digest,
    "schema_version": 1,
    "tool_count": 41,
    "tools": records,
}}
print(json.dumps(value, sort_keys=True, separators=(",", ":")))
'''.encode()


def provision_future_archived_python_runtime(source: Path, root: Path) -> None:
    """Provision the framework companion for ``root/evidence/python3``."""

    provision_archived_python_runtime(source, root / "evidence" / "python3")


def provision_rebound_archived_python_runtime(
    label: str, source: Path, archive: Path
) -> None:
    """Provision a rebound Python archive and ignore every other tool label."""

    if label == "python":
        provision_archived_python_runtime(source, archive)


def provision_archived_python_runtime(source: Path, archive: Path) -> None:
    """Provide the complete runtime needed by a copied macOS Python launcher."""

    if sys.platform != "darwin":
        return
    if source.resolve(strict=True) != Path(sys.executable).resolve(strict=True):
        return
    framework_name = sysconfig.get_config_var("PYTHONFRAMEWORK")
    if not isinstance(framework_name, str) or not framework_name:
        return

    source = source.resolve(strict=True)
    version_root = source.parent.parent
    framework_binary = version_root / framework_name
    framework_resources = version_root / "Resources"
    libdest_value = sysconfig.get_config_var("LIBDEST")
    if not isinstance(libdest_value, str) or not libdest_value:
        return
    framework_stdlib = Path(libdest_value).resolve(strict=True)
    if (
        not framework_binary.is_file()
        or framework_binary.is_symlink()
        or not framework_resources.is_dir()
        or framework_resources.is_symlink()
        or not framework_stdlib.is_dir()
        or framework_stdlib.is_symlink()
        or framework_stdlib.parent != version_root / "lib"
    ):
        return

    # The archived launcher lives at ``<root>/evidence/python3``. Apple
    # framework builds load ``@executable_path/../<framework>`` and then use
    # the adjacent Resources/Python.app trampoline. Prefix discovery also
    # requires the exact ``lib/pythonX.Y`` landmark; without it the copied
    # executable silently imports from the caller's framework installation.
    # Keep this test layout aligned with the production archive contract.
    runtime_root = archive.parent.parent
    archived_framework = runtime_root / framework_name
    archived_resources = runtime_root / "Resources"
    archived_stdlib = runtime_root / "lib" / framework_stdlib.name
    if (
        archived_framework.exists()
        or archived_resources.exists()
        or archived_stdlib.exists()
    ):
        assert archived_framework.is_file()
        assert not archived_framework.is_symlink()
        assert archived_framework.read_bytes() == framework_binary.read_bytes()
        assert _tree_members(archived_resources) == _tree_members(
            framework_resources
        )
        assert _tree_members(archived_stdlib) == _tree_members(
            framework_stdlib,
            omitted_paths=_FRAMEWORK_STDLIB_OMITTED_PATHS,
        )
        return

    shutil.copyfile(framework_binary, archived_framework)
    archived_framework.chmod(0o500)
    shutil.copytree(framework_resources, archived_resources, symlinks=True)
    archived_stdlib.parent.mkdir(mode=0o700)
    shutil.copytree(
        framework_stdlib,
        archived_stdlib,
        symlinks=True,
        ignore=lambda directory, names: (
            _FRAMEWORK_STDLIB_OMITTED_PATHS.intersection(names)
            if Path(directory) == framework_stdlib
            else frozenset()
        ),
    )


def _tree_members(
    root: Path,
    *,
    omitted_paths: frozenset[str] = frozenset(),
) -> dict[str, tuple[object, ...]]:
    """Return an exact no-follow member inventory for one fixture tree."""

    assert root.is_dir() and not root.is_symlink()
    records: dict[str, tuple[object, ...]] = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root).as_posix()
        if any(
            relative == omitted or relative.startswith(f"{omitted}/")
            for omitted in omitted_paths
        ):
            continue
        metadata = path.lstat()
        if path.is_symlink():
            records[relative] = ("symlink", os.readlink(path))
        elif path.is_dir():
            records[relative] = ("directory",)
        elif path.is_file():
            records[relative] = (
                "file",
                metadata.st_size,
                hashlib.sha256(path.read_bytes()).hexdigest(),
            )
        else:
            raise AssertionError(f"framework Python fixture has special member: {path}")
    return records


def runner_tool_manifest(tool_root: Path) -> bytes:
    """Return the canonical manifest for the host tools trusted by the test runner."""
    tools: dict[str, dict[str, str]] = {}
    for name in REQUIRED_RUNNER_TOOL_NAMES:
        if name in {"cargo", "rustc"}:
            rustup = shutil.which("rustup", path=os.environ.get("PATH", ""))
            assert rustup is not None
            selected = subprocess.run(
                [rustup, "which", name],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            ).stdout.strip()
            discovered = selected or None
        elif name.startswith("git-"):
            git = shutil.which("git", path=os.defpath)
            assert git is not None
            exec_path = subprocess.run(
                [git, "--exec-path"], check=True, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, text=True,
            ).stdout.strip()
            source = (Path(exec_path) / name).resolve(strict=True)
            private_tool = tool_root / f"runner-{name}"
            shutil.copyfile(source, private_tool)
            private_tool.chmod(0o500)
            discovered = str(private_tool)
        else:
            discovered = shutil.which(name, path=os.defpath)
        if discovered is None:
            private_tool = tool_root / f"runner-{name}"
            private_tool.write_bytes(b"#!/bin/sh\nexit 0\n")
            private_tool.chmod(0o500)
            discovered = str(private_tool)
        path = Path(discovered).resolve(strict=True)
        tools[name] = {
            "path": str(path),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
    return (
        json.dumps(
            {"schema_version": 1, "tools": tools},
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode()
