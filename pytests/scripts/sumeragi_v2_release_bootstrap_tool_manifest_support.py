"""Build canonical runner-tool manifests for Sumeragi bootstrap tests."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess


def runner_tool_manifest() -> bytes:
    """Return the canonical manifest for the host tools trusted by the test runner."""
    tools: dict[str, dict[str, str]] = {}
    for name in ("chmod", "ln", "mv", "sleep", "cargo", "rustc"):
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
