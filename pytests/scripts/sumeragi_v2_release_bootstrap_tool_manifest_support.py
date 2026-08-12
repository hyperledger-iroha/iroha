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
    """Provide the external runtime needed by a copied macOS Python launcher."""

    if sys.platform != "darwin":
        return
    if source.resolve(strict=True) != Path(sys.executable).resolve(strict=True):
        return
    framework_name = sysconfig.get_config_var("PYTHONFRAMEWORK")
    if not isinstance(framework_name, str) or not framework_name:
        return

    version_root = source.parent.parent
    framework_binary = version_root / framework_name
    framework_resources = version_root / "Resources"
    if not framework_binary.is_file() or not framework_resources.is_dir():
        return

    # The archived launcher lives at ``<root>/evidence/python3``. Apple
    # framework builds load ``@executable_path/../<framework>`` and then use
    # the adjacent Resources/Python.app trampoline. Those loader inputs are an
    # external bootstrap prerequisite, so keep their test copies outside both
    # the candidate and the authenticated evidence directory.
    runtime_root = archive.parent.parent
    archived_framework = runtime_root / framework_name
    archived_resources = runtime_root / "Resources"
    if archived_framework.exists() or archived_resources.exists():
        assert archived_framework.is_file()
        assert not archived_framework.is_symlink()
        assert archived_framework.read_bytes() == framework_binary.read_bytes()
        archived_trampoline = (
            archived_resources / "Python.app" / "Contents" / "MacOS" / "Python"
        )
        source_trampoline = (
            framework_resources / "Python.app" / "Contents" / "MacOS" / "Python"
        )
        assert archived_trampoline.is_file()
        assert archived_trampoline.read_bytes() == source_trampoline.read_bytes()
        return

    shutil.copyfile(framework_binary, archived_framework)
    archived_framework.chmod(0o500)
    shutil.copytree(framework_resources, archived_resources)


def runner_tool_manifest(tool_root: Path) -> bytes:
    """Return the canonical manifest for the host tools trusted by the test runner."""
    tools: dict[str, dict[str, str]] = {}
    for name in (
        "chmod", "ln", "mv", "sleep", "cargo", "rustc",
        "git-upload-pack", "git-index-pack",
    ):
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
        assert discovered is not None
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
