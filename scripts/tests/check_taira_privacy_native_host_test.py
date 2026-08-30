from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).parents[2]
CHECKER = ROOT / "ci/check_taira_privacy_native_host.sh"
CONTROLLER = ROOT / "scripts/capture_zk_x509_native_candidate.py"
PACKAGER = ROOT / "scripts/package_zk_x509_prover_worker.py"
VERIFIER = ROOT / "scripts/verify_ec2_instance_identity.py"


def _load_script(path: Path, name: str):
    specification = importlib.util.spec_from_file_location(name, path)
    assert specification is not None and specification.loader is not None
    module = importlib.util.module_from_spec(specification)
    sys.modules[name] = module
    specification.loader.exec_module(module)
    return module


def test_checker_is_valid_bash_and_documents_every_absolute_tool_role() -> None:
    subprocess.run(["/bin/bash", "-n", str(CHECKER)], check=True)
    completed = subprocess.run(
        ["/bin/bash", str(CHECKER), "--help"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    for option in ("--python", "--lscpu", "--uname", "--grep", "--tr", "--probe-root"):
        assert option in completed.stdout


def test_checker_cannot_fall_back_to_path_or_recursive_cleanup(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    (fake_bin / "python3").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    (fake_bin / "python3").chmod(0o700)
    completed = subprocess.run(
        ["/bin/bash", str(CHECKER)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env={"PATH": str(fake_bin)},
    )
    assert completed.returncode != 0
    assert "--python must name an absolute executable" in completed.stderr

    source = CHECKER.read_text(encoding="utf-8")
    for forbidden in (
        "command -v",
        "systemd-detect-virt",
        "mktemp",
        "rm -rf",
        "cat <<",
        "dirname --",
        "python3 -I",
        "lscpu --json",
        "$(uname ",
        "$(tr ",
        "| grep ",
    ):
        assert forbidden not in source
    for invocation in (
        '"$python_path" -I -S',
        '"$lscpu_path" --json',
        '"$uname_path" -s',
        '"$grep_path" -E',
        '"$tr_path" -d',
    ):
        assert invocation in source
    for containment_contract in (
        "CLONE_NEWUSER",
        "CLONE_NEWPID",
        'write_once("/proc/self/uid_map"',
        'write_once("/proc/self/gid_map"',
        "Linux trusted PID-namespace init qualification failed",
    ):
        assert containment_contract in source


def test_controller_and_packager_bind_the_complete_host_tool_contract() -> None:
    controller = CONTROLLER.read_text(encoding="utf-8")
    packager = PACKAGER.read_text(encoding="utf-8")
    for role in ("python", "lscpu", "uname", "grep", "tr"):
        assert f'"{role}"' in packager
        assert f'toolchain_before["tools"]["{role}"]' in controller
    for option in ("--python", "--lscpu", "--uname", "--grep", "--tr", "--probe-root"):
        assert f'"{option}"' in controller
    assert '"cargo_iroha_fast"' not in packager
    assert '"iroha-fast"' not in controller
    assert "shutil.rmtree" not in controller
    assert "host_probe_root = make_fresh_directory(" in controller


def test_containment_supervisor_source_is_identical_across_all_runners() -> None:
    sources = {
        _load_script(path, f"containment_identity_{index}")._LINUX_PID_NAMESPACE_SUPERVISOR
        for index, path in enumerate((PACKAGER, CONTROLLER, VERIFIER))
    }
    assert len(sources) == 1
    compile(sources.pop(), "<zk-x509-linux-containment-supervisor>", "exec")
