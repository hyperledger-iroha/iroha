"""Exercise bounded hosted swap provisioning without privileged host mutations."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import stat
import subprocess
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("hosted_build_swap", ROOT / "ci/provision_hosted_build_swap.py")
assert SPEC and SPEC.loader
swap = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(swap)


@pytest.fixture
def hosted(monkeypatch, tmp_path):
    monkeypatch.setattr(swap.sys, "platform", "linux")
    for key, value in {"GITHUB_ACTIONS": "true", "RUNNER_ENVIRONMENT": "github-hosted",
                       "RUNNER_OS": "Linux", "RUNNER_TEMP": str(tmp_path)}.items():
        monkeypatch.setenv(key, value)
    monkeypatch.setattr(swap.shutil, "disk_usage", lambda path: SimpleNamespace(
        free=swap.SWAP_BYTES + swap.BUILD_HEADROOM_BYTES))
    calls = []

    def run(arguments, *, check, stdout=None):
        assert check is True
        calls.append(arguments)
        if arguments[0] == "dd":
            assert stdout is not None
            # A sparse fixture verifies extent accounting; no host swap is enabled.
            stdout.truncate(swap.SWAP_BYTES)
        else:
            assert stdout is None
        return subprocess.CompletedProcess(arguments, 0)

    monkeypatch.setattr(swap.subprocess, "run", run)
    return tmp_path, calls, run


def test_adds_one_bounded_file_and_preserves_existing_swap(hosted):
    root, calls, _ = hosted
    existing = root / "existing-swap"
    existing.write_bytes(b"existing")
    path = swap.provision()
    assert swap.SWAP_BYTES == 16 * 1024**3
    assert swap.BUILD_HEADROOM_BYTES == 24 * 1024**3
    assert path == root / "iroha-ci-build-swap/swapfile"
    assert stat.S_IMODE(path.stat().st_mode) == 0o600
    assert stat.S_IMODE(path.parent.stat().st_mode) == 0o700
    assert path.stat().st_size == swap.SWAP_BYTES
    assert existing.read_bytes() == b"existing"
    assert calls == [
        ["dd", "if=/dev/zero", "bs=1M", "count=16384", "conv=fsync", "status=none"],
        ["sudo", "-n", "chown", "0:0", "--", str(path)],
        ["sudo", "-n", "mkswap", "--", str(path)],
        ["sudo", "-n", "swapon", "--", str(path)],
        ["swapon", "--show", "--bytes"],
    ]
    with pytest.raises(FileExistsError):
        swap.provision()
    assert len(calls) == 5


@pytest.mark.parametrize("key,value", [("GITHUB_ACTIONS", "false"),
    ("RUNNER_ENVIRONMENT", "self-hosted"), ("RUNNER_OS", "Windows")])
def test_refuses_other_runner_environments(hosted, monkeypatch, key, value):
    root, calls, _ = hosted
    monkeypatch.setenv(key, value)
    with pytest.raises(RuntimeError, match="GitHub-hosted Linux"):
        swap.provision()
    assert not calls and not list(root.iterdir())


def test_refuses_non_linux_host(hosted, monkeypatch):
    root, calls, _ = hosted
    monkeypatch.setattr(swap.sys, "platform", "darwin")
    with pytest.raises(RuntimeError, match="GitHub-hosted Linux"):
        swap.provision()
    assert not calls and not list(root.iterdir())


@pytest.mark.parametrize("value", ["", "relative", "/definitely-missing-iroha-swap-parent"])
def test_refuses_invalid_temporary_directory(hosted, monkeypatch, value):
    _, calls, _ = hosted
    monkeypatch.setenv("RUNNER_TEMP", value)
    with pytest.raises(RuntimeError, match="temporary directory"):
        swap.provision()
    assert not calls


def test_refuses_insufficient_disk_before_allocation(hosted, monkeypatch):
    root, calls, _ = hosted
    monkeypatch.setattr(swap.shutil, "disk_usage", lambda path: SimpleNamespace(
        free=swap.SWAP_BYTES + swap.BUILD_HEADROOM_BYTES - 1))
    with pytest.raises(RuntimeError, match="build headroom"):
        swap.provision()
    assert not calls and not list(root.iterdir())


def test_rechecks_headroom_before_formatting_and_activation(hosted, monkeypatch):
    _, calls, _ = hosted
    free = iter([swap.SWAP_BYTES + swap.BUILD_HEADROOM_BYTES,
                 swap.BUILD_HEADROOM_BYTES - 1])
    monkeypatch.setattr(swap.shutil, "disk_usage", lambda path: SimpleNamespace(free=next(free)))
    with pytest.raises(RuntimeError, match="left insufficient build headroom"):
        swap.provision()
    assert len(calls) == 1 and calls[0][0] == "dd"


@pytest.mark.parametrize("entry", ["file", "directory", "symlink"])
def test_refuses_existing_output_without_touching_it(hosted, entry):
    root, calls, _ = hosted
    output = root / "iroha-ci-build-swap"
    sentinel = root / "sentinel"
    sentinel.write_bytes(b"retained")
    if entry == "file":
        output.write_bytes(b"retained")
    elif entry == "directory":
        output.mkdir()
    else:
        output.symlink_to(sentinel)
    with pytest.raises(FileExistsError):
        swap.provision()
    assert not calls and sentinel.read_bytes() == b"retained"


@pytest.mark.parametrize("failed", ["dd", "chown", "mkswap", "swapon"])
def test_provisioning_failure_stops_before_later_effects(hosted, monkeypatch, failed):
    _, calls, run = hosted

    def fail(arguments, **kwargs):
        if arguments[0] == failed or (arguments[0] == "sudo" and arguments[2] == failed):
            raise subprocess.CalledProcessError(23, arguments)
        return run(arguments, **kwargs)

    monkeypatch.setattr(swap.subprocess, "run", fail)
    with pytest.raises(subprocess.CalledProcessError) as error:
        swap.provision()
    assert error.value.returncode == 23
    assert len(calls) == ["dd", "chown", "mkswap", "swapon"].index(failed)


def test_short_write_never_formats_or_activates(hosted, monkeypatch):
    _, calls, _ = hosted

    def short(arguments, *, stdout, check):
        calls.append(arguments)
        stdout.truncate(swap.SWAP_BYTES - 1)

    monkeypatch.setattr(swap.subprocess, "run", short)
    with pytest.raises(RuntimeError, match="unexpected extent"):
        swap.provision()
    assert len(calls) == 1 and calls[0][0] == "dd"


def test_only_observed_build_owners_provision_before_their_work():
    workflow = (ROOT / ".github/workflows/pr.yml").read_text()
    command = "python3 -I -S ci/provision_hosted_build_swap.py"
    blocks = re.split(r"(?m)^  ([a-z_0-9]+):\n", workflow.split("jobs:\n", 1)[1])
    jobs = dict(zip(blocks[1::2], blocks[2::2]))
    selected = {name for name, body in jobs.items() if command in body}
    observed = {"pre_build", "sumeragi", "zk_phase_tree_measurement", "x2_maximum_proof_transport"}
    assert selected == observed | {"rust_affected", "sora_parliament_lifecycle"}
    assert workflow.count(command) == 6
    for name in observed:
        body = jobs[name]
        assert "runs-on: ubuntu-24.04" in body
        assert body.index(command) < body.index("run: cargo test" if name == "x2_maximum_proof_transport"
                                              else "ci/observe_build_capacity.py")
    generic = jobs["rust_affected"]
    assert "runs-on: ubuntu-24.04" in generic
    step = generic.split("      - name: Provision bounded hosted build swap for Core and execution checks\n", 1)[1]
    step = step.split("\n      - ", 1)[0]
    # Comma boundaries select the exact router package, including first/last positions;
    # similarly named packages such as iroha_core_privacy cannot match the Core arm.
    assert step == (
        "        if: matrix.lane == 'execution' || contains(format(',{0},', matrix.packages), ',iroha_core,')\n"
        f"        run: {command}"
    )
    assert generic.index(command) < generic.index("      - name: Run locked affected checks")
    parliament = jobs["sora_parliament_lifecycle"]
    assert "runs-on: ubuntu-24.04" in parliament
    assert "timeout-minutes: 120" in parliament
    assert parliament.index("Verify isolated Parliament corridor source contract") < parliament.index(command)
    step = parliament.split("      - name: Provision bounded hosted build swap for Parliament Core tests\n", 1)[1]
    assert step.split("\n      - ", 1)[0] == f"        run: {command}"
    assert parliament.index(command) < parliament.index("ci/observe_build_capacity.py -- bash ci/check_sora_parliament_lifecycle.sh")
    assert "scripts/tests/hosted_build_swap_test.py" in jobs["rust_changes"]


@pytest.mark.parametrize("lane,packages,expected", [
    ("execution", "ivm,iroha_core_privacy", True),
    ("execution", "iroha_plonk_recursion", True),
    ("node", "iroha_core", True),
    ("node", "iroha_core,iroha", True),
    ("node", "iroha,iroha_core,irohad", True),
    ("node", "iroha,iroha_core", True),
    ("node", "iroha_core_privacy,iroha_core_zk", False),
    ("hardware", "gpuzstd_cuda", False),
    ("tooling", "xtask", False),
    ("execution_extra", "ivm", False),
])
def test_affected_swap_condition_selects_exact_observed_owners(lane, packages, expected):
    """Evaluate the actual workflow predicate, including package and lane boundaries."""
    workflow = (ROOT / ".github/workflows/pr.yml").read_text()
    step = workflow.split(
        "      - name: Provision bounded hosted build swap for Core and execution checks\n", 1,
    )[1].split("\n      - ", 1)[0]
    condition = step.split("        if: ", 1)[1].splitlines()[0]
    # Translate only this predicate's GitHub expression operators and matrix fields.
    # The exact condition is separately fixed by the owner-routing contract above.
    expression = (condition.replace(" || ", " or ")
                  .replace("matrix.lane", "lane").replace("matrix.packages", "packages"))
    selected = eval(expression, {"__builtins__": {}}, {
        "lane": lane, "packages": packages,
        "contains": lambda value, needle: needle in value,
        "format": lambda template, value: template.format(value),
    })
    assert selected is expected
