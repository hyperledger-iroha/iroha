"""Preserve SM OpenSSL smoke prerequisites, compiler failures, and target selection."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "sm_openssl_smoke.sh"


def run_smoke(
    tmp_path: Path,
    *,
    have_cargo: bool = True,
    have_pkg_config: bool = True,
    pkg_status: int = 0,
    check_status: int = 0,
    test_status: int = 0,
) -> tuple[subprocess.CompletedProcess[str], list[dict[str, object]]]:
    """Use isolated tool stubs; no real compiler or cryptographic backend executes."""

    bash = shutil.which("bash")
    dirname = shutil.which("dirname")
    assert bash and dirname
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    (fake_bin / "dirname").symlink_to(dirname)
    log = tmp_path / "cargo.jsonl"
    if have_cargo:
        cargo = fake_bin / "cargo"
        cargo.write_text(
            f"#!{sys.executable}\n"
            "import json, os, sys\n"
            "with open(os.environ['SM_SMOKE_TEST_LOG'], 'a') as output:\n"
            "    output.write(json.dumps({'args': sys.argv[1:], 'rustflags': os.environ.get('RUSTFLAGS')}) + '\\n')\n"
            "assert sys.argv[1] in ('check', 'test')\n"
            "raise SystemExit(int(os.environ['SM_SMOKE_TEST_' + sys.argv[1].upper() + '_STATUS']))\n",
            encoding="utf-8",
        )
        cargo.chmod(0o755)
    if have_pkg_config:
        pkg = fake_bin / "pkg-config"
        pkg.write_text(
            f"#!{sys.executable}\n"
            "import os, sys\n"
            "assert sys.argv[1:] == ['--exists', 'openssl >= 3.0.0']\n"
            "raise SystemExit(int(os.environ['SM_SMOKE_TEST_PKG_STATUS']))\n",
            encoding="utf-8",
        )
        pkg.chmod(0o755)
    env = os.environ.copy()
    env.update(
        PATH=str(fake_bin),
        RUSTFLAGS="--cfg sm_smoke_test",
        SM_SMOKE_TEST_LOG=str(log),
        SM_SMOKE_TEST_CHECK_STATUS=str(check_status),
        SM_SMOKE_TEST_TEST_STATUS=str(test_status),
        SM_SMOKE_TEST_PKG_STATUS=str(pkg_status),
    )
    result = subprocess.run(
        [bash, str(SCRIPT), "--offline"],
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
    return result, calls


@pytest.mark.parametrize(
    ("options", "status", "diagnostic"),
    [
        ({"have_cargo": False}, 1, "cargo not found"),
        ({"have_pkg_config": False}, 2, "pkg-config not found"),
        ({"pkg_status": 1}, 2, "development files not detected"),
    ],
)
def test_missing_prerequisites_never_report_success_or_run_cargo(
    tmp_path: Path, options: dict[str, object], status: int, diagnostic: str
) -> None:
    result, calls = run_smoke(tmp_path, **options)
    assert result.returncode == status
    assert diagnostic in result.stderr
    assert calls == []


def test_compiler_failure_stops_before_tests_with_original_status(tmp_path: Path) -> None:
    result, calls = run_smoke(tmp_path, check_status=17)
    assert result.returncode == 17
    assert [call["args"][0] for call in calls] == ["check"]


@pytest.mark.parametrize("test_status", [0, 19])
def test_group_target_preserves_test_status_arguments_and_rustflags(
    tmp_path: Path, test_status: int
) -> None:
    result, calls = run_smoke(tmp_path, test_status=test_status)
    assert result.returncode == test_status
    common = [
        "--locked",
        "--manifest-path",
        "crates/iroha_crypto/Cargo.toml",
        "--features",
        "sm sm-ffi-openssl",
    ]
    assert [call["args"] for call in calls] == [
        ["check", *common, "--offline"],
        [
            "test", *common,
            "--test", "iroha_crypto_group_01", "--offline",
            "sm_openssl_smoke::", "--", "--nocapture",
        ],
    ]
    assert [call["rustflags"] for call in calls] == ["--cfg sm_smoke_test"] * 2
