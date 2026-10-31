#!/usr/bin/env python3
"""Check that this machine can build Iroha and run the contributor checks.

Requires only a Python 3 interpreter and changes nothing. Run it with `make doctor`.
It exits 1 when something the contributor workflow needs is missing, and prints the
command that fixes each problem. Optional tools only produce notes.
"""

from __future__ import annotations

import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from typing import Callable, List, NamedTuple, Optional, Sequence, Tuple


ROOT = Path(__file__).resolve().parent.parent
# Repository scripts import tomllib; CI runs 3.11 and 3.12.
MINIMUM_PYTHON = (3, 11)
SWIFT_PYTHON = (3, 12)
PYTHON_MODULES = ("blake3", "jsonschema", "pytest", "requests", "tomli_w")
# One debug target directory for the daemon, CLI and Kagami used about 54 GiB.
MINIMUM_FREE_BYTES = 100 * 1024 * 1024 * 1024
OPTIONAL_TOOLS = (
    ("sccache", "faster cold builds"),
    ("docker", "kagami docker bundles"),
    ("java", "Kotlin SDK (JDK 21)"),
    ("swift", "Swift SDK"),
)

OK = "ok"
FAIL = "FAIL"
WARN = "warn"
NOTE = "note"


class Finding(NamedTuple):
    status: str
    name: str
    detail: str
    fix: str = ""


Which = Callable[[str], Optional[str]]
Run = Callable[[Sequence[str]], Tuple[int, str]]


def run_command(command: Sequence[str]) -> Tuple[int, str]:
    """Run one read-only probe from the repository root."""
    try:
        done = subprocess.run(
            list(command),
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            universal_newlines=True,
            timeout=60,
        )
    except (OSError, subprocess.SubprocessError) as error:
        return 127, str(error)
    return done.returncode, done.stdout.strip()


def pinned_toolchain(root: Path = ROOT) -> Optional[str]:
    """Read the channel pinned by rust-toolchain.toml."""
    try:
        text = (root / "rust-toolchain.toml").read_text(encoding="utf-8")
    except OSError:
        return None
    match = re.search(r'^\s*channel\s*=\s*"([^"]+)"', text, re.MULTILINE)
    return match.group(1) if match else None


def configured_base_ref(root: Path = ROOT) -> Optional[str]:
    """Read the integration branch from ci/default_base_ref."""
    try:
        lines = (root / "ci" / "default_base_ref").read_text(encoding="utf-8").splitlines()
    except OSError:
        return None
    for line in lines:
        value = line.split("#", 1)[0].strip()
        if value:
            return value
    return None


def check_rust(which: Which, run: Run, channel: Optional[str]) -> List[Finding]:
    if which("rustup") is None:
        return [Finding(FAIL, "rustup", "not found", "install it from https://rustup.rs")]
    if channel is None:
        return [Finding(FAIL, "toolchain", "rust-toolchain.toml names no channel")]
    code, installed = run(("rustup", "toolchain", "list"))
    if code != 0 or not any(line.startswith(channel) for line in installed.splitlines()):
        return [
            Finding(
                FAIL,
                "toolchain",
                f"Rust {channel} is not installed",
                f"rustup toolchain install {channel} --component rustfmt --component clippy",
            )
        ]
    findings = [Finding(OK, "toolchain", f"Rust {channel}")]
    code, components = run(("rustup", "component", "list", "--toolchain", channel, "--installed"))
    for component in ("rustfmt", "clippy"):
        if code != 0 or not any(line.startswith(component) for line in components.splitlines()):
            findings.append(
                Finding(
                    FAIL,
                    component,
                    "not installed",
                    f"rustup component add {component} --toolchain {channel}",
                )
            )
    return findings


def check_ripgrep(which: Which) -> Finding:
    if which("rg") is None:
        return Finding(
            FAIL,
            "rg",
            "not found; the serialization and security guards need it",
            "brew install ripgrep, apt install ripgrep or cargo install ripgrep",
        )
    return Finding(OK, "rg", "found")


def parse_python_version(text: str) -> Optional[Tuple[int, int]]:
    match = re.search(r"(\d+)\.(\d+)", text)
    return (int(match.group(1)), int(match.group(2))) if match else None


def check_python(which: Which, run: Run) -> List[Finding]:
    if which("python3") is None:
        return [Finding(FAIL, "python3", "not found", "install Python 3.12")]
    code, output = run(("python3", "--version"))
    version = parse_python_version(output) if code == 0 else None
    if version is None:
        return [Finding(FAIL, "python3", f"cannot read its version: {output}")]
    label = ".".join(str(part) for part in version)
    minimum = ".".join(str(part) for part in MINIMUM_PYTHON)
    if version < MINIMUM_PYTHON:
        return [
            Finding(
                FAIL,
                "python3",
                f"{label} is below the {minimum} the repository scripts need",
                "install Python 3.12 (brew install python@3.12) and put it first on PATH",
            )
        ]
    findings = [Finding(OK, "python3", label)]
    probe = "; ".join(f"import {module}" for module in PYTHON_MODULES)
    code, output = run(("python3", "-c", probe))
    if code != 0:
        findings.append(
            Finding(
                WARN,
                "python packages",
                output.splitlines()[-1] if output else "missing",
                "python3 -m pip install -r scripts/requirements.txt",
            )
        )
    return findings


def check_base_ref(run: Run, configured: Optional[str]) -> Finding:
    if configured is None:
        return Finding(WARN, "base ref", "ci/default_base_ref names no branch")
    code, _ = run(("git", "rev-parse", "--verify", "--quiet", configured))
    if code != 0:
        branch = configured.split("/", 1)[-1]
        return Finding(
            WARN,
            "base ref",
            f"{configured} is missing, so the guards fall back to another branch",
            f"git fetch origin {branch}",
        )
    return Finding(OK, "base ref", configured)


def check_disk(free_bytes: int) -> Finding:
    gib = free_bytes // (1024 * 1024 * 1024)
    if free_bytes < MINIMUM_FREE_BYTES:
        return Finding(
            WARN,
            "disk",
            f"{gib} GiB free; one debug target directory uses about 54 GiB",
            "free some space or set CARGO_TARGET_DIR to a larger volume",
        )
    return Finding(OK, "disk", f"{gib} GiB free")


def check_localnet_binaries(target: Path) -> Optional[Finding]:
    """`kagami localnet up` needs a matching iroha3d beside it."""
    have_kagami = (target / "debug" / "kagami").exists()
    have_daemon = (target / "debug" / "iroha3d").exists()
    if have_kagami == have_daemon:
        return None
    return Finding(
        WARN,
        "localnet",
        f"{target / 'debug'} has only {'kagami' if have_kagami else 'iroha3d'}",
        "cargo build -p iroha_kagami -p irohad",
    )


def check_swift_python(which: Which) -> Optional[Finding]:
    if which("swift") is None:
        return None
    version = f"python{SWIFT_PYTHON[0]}.{SWIFT_PYTHON[1]}"
    if which(version) is None:
        return Finding(NOTE, version, "not found; scripts/test_swift_local.py requires it")
    return None


def collect(which: Which = shutil.which, run: Run = run_command, root: Path = ROOT) -> List[Finding]:
    findings = [
        Finding(OK, "git", "found") if which("git") else Finding(FAIL, "git", "not found"),
    ]
    findings.extend(check_rust(which, run, pinned_toolchain(root)))
    findings.append(check_ripgrep(which))
    findings.extend(check_python(which, run))
    findings.append(check_base_ref(run, configured_base_ref(root)))
    findings.append(check_disk(shutil.disk_usage(str(root)).free))
    target = Path(os.environ.get("CARGO_TARGET_DIR") or root / "target")
    for finding in (check_localnet_binaries(target), check_swift_python(which)):
        if finding is not None:
            findings.append(finding)
    for tool, purpose in OPTIONAL_TOOLS:
        if which(tool) is None:
            findings.append(Finding(NOTE, tool, f"not found; only needed for {purpose}"))
    return findings


def render(finding: Finding) -> str:
    line = f"{finding.status:<4} {finding.name}: {finding.detail}"
    return f"{line}\n     fix: {finding.fix}" if finding.fix else line


def main() -> int:
    findings = collect()
    print("\n".join(render(finding) for finding in findings))
    failed = sum(finding.status == FAIL for finding in findings)
    if failed:
        print(f"\n{failed} required tool(s) missing.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
