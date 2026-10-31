"""Select a native fixture shell without dispatching Windows paths through WSL.

This chooses test tooling only; it does not authenticate a release executable.
"""

from __future__ import annotations

import os
from pathlib import Path
import shutil


def bash_executable() -> str:
    """Require Git for Windows on Windows; preserve ordinary Bash lookup on Unix."""
    if os.name == "nt":
        program_files = Path(os.environ.get("ProgramFiles", "C:/Program Files"))
        candidate = program_files / "Git/bin/bash.exe"
        if not candidate.is_file():
            raise FileNotFoundError(f"native Git Bash fixture host is required: {candidate}")
        return str(candidate)
    candidate = shutil.which("bash")
    if candidate is None:
        raise FileNotFoundError("Bash fixture host is required")
    return candidate
