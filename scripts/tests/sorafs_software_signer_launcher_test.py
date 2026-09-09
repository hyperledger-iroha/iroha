"""Execute the shipped launcher against an isolated child, never a deployed signer."""
from __future__ import annotations

import json
import plistlib
import shlex
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
LAUNCHD = ROOT / "configs/sorafs/external_software_signer/launchd"
LAUNCHER = LAUNCHD / "sorafs-external-software-signer-launchd-v1"
ROLES = (
    "proof-outcome", "repair", "reserve", "orderbook", "governance-dag",
    "potr-gateway", "potr-provider", "billing", "evidence-viewer", "pop-credentials",
)


@pytest.mark.parametrize("arguments", [
    [], ["stream-token"], ["release-manifest"], ["STREAM-TOKEN"],
    ["../stream-token"], ["stream-token", "evidence-viewer"],
    ["evidence-viewer", "stream-token"],
])
def test_forbidden_or_ambiguous_role_fails_before_child_or_credential_use(arguments: list[str]) -> None:
    result = subprocess.run(
        ["/bin/sh", str(LAUNCHER), *arguments], input=b"",
        capture_output=True, timeout=5, check=False,
    )
    assert result.returncode == 64
    assert result.stdout == b""
    assert result.stderr in (
        b"unsupported signer role\n", b"exactly one fixed signer role is required\n",
    )


@pytest.mark.parametrize("role", ROLES)
def test_remaining_roles_keep_exact_paths_and_inherited_descriptor(tmp_path: Path, role: str) -> None:
    child = tmp_path / "fake_signer.py"
    child.write_text(
        "import json, os, sys\n"
        "assert os.read(3, 64) == b'isolated-test-descriptor'\n"
        "print(json.dumps(sys.argv[1:]))\n",
        encoding="utf-8",
    )
    script = LAUNCHER.read_text(encoding="utf-8")
    fixed_exec = "exec /usr/local/bin/sorafs_external_software_signer serve"
    assert script.count(fixed_exec) == 1
    # Substitute only the fixed child executable; the shipped role guard, fd duplication and
    # all argument/path construction run verbatim. This cannot start an installed service.
    isolated = tmp_path / "launcher"
    isolated.write_text(script.replace(
        fixed_exec, f"exec {shlex.quote(sys.executable)} {shlex.quote(str(child))} serve",
    ), encoding="utf-8")
    result = subprocess.run(
        ["/bin/sh", str(isolated), role], input=b"isolated-test-descriptor",
        capture_output=True, timeout=5, check=False,
    )
    assert result.returncode == 0, result.stderr
    assert result.stderr == b""
    assert json.loads(result.stdout) == [
        "serve", "--state-directory", f"/private/var/db/iroha/sorafs-signers/{role}",
        "--binding", f"/private/etc/sorafs/signers/{role}.binding.norito",
        "--request-socket", f"/private/var/iroha/sorafs-signers/{role}/request.sock",
        "--administrator-socket", f"/private/var/iroha/sorafs-signers/{role}/administrator.sock",
        "--wrapping-key-fd", "3",
    ]


def test_shipped_launchd_inventory_contains_exactly_the_remaining_roles() -> None:
    jobs = sorted(LAUNCHD.glob("org.hyperledger.iroha.sorafs-signer-*.plist"))
    assert len(jobs) == len(ROLES)
    actual_roles = set()
    for path in jobs:
        with path.open("rb") as source:
            job = plistlib.load(source)
        arguments = job["ProgramArguments"]
        assert len(arguments) == 2
        assert arguments[0] == "/usr/local/libexec/sorafs-external-software-signer-launchd-v1"
        role = arguments[1]
        actual_roles.add(role)
        assert job["Label"] == f"org.hyperledger.iroha.sorafs-signer-{role}"
        assert job["UserName"] == f"sorafs-signer-{role}"
        assert job["GroupName"] == f"sorafs-signer-{role}"
        assert job["StandardInPath"] == f"/private/var/run/iroha-signer-credentials/{role}.wrapping-key"
    assert actual_roles == set(ROLES)
