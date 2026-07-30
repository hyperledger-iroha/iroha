"""Regression tests for the Prometheus rule validation wrapper."""

from __future__ import annotations

import os
import stat
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CHECKER = REPO_ROOT / "scripts" / "check_prometheus_rules.sh"


def write_fake_executable(path: Path, body: str) -> None:
    """Write an executable fake command for wrapper contract tests."""

    path.write_text(f"#!/usr/bin/env bash\nset -euo pipefail\n{body}", encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


def checker_env(bin_dir: Path, **extra: str) -> dict[str, str]:
    """Return an environment that resolves the test promtool first."""

    return {
        **os.environ,
        **extra,
        "PATH": f"{bin_dir}:/usr/bin:/bin",
    }


def test_checks_every_supplied_rule_file(tmp_path: Path) -> None:
    """Every wildcard-expanded rule path must reach one promtool invocation."""

    args_out = tmp_path / "promtool-args"
    write_fake_executable(
        tmp_path / "promtool",
        'printf "%s\\n" "$@" > "${PROMTOOL_ARGS_OUT}"\n',
    )
    rule_files = [
        "dashboards/alerts/sorafs_fetch_rules.yml",
        "dashboards/alerts/sorafs_gateway_rules.yml",
    ]

    subprocess.run(
        [str(CHECKER), *rule_files],
        cwd=REPO_ROOT,
        env=checker_env(tmp_path, PROMTOOL_ARGS_OUT=str(args_out)),
        check=True,
        text=True,
        capture_output=True,
    )

    assert args_out.read_text(encoding="utf-8").splitlines() == [
        "check",
        "rules",
        *(str(REPO_ROOT / rule_file) for rule_file in rule_files),
    ]


def test_missing_rule_fails_before_promtool(tmp_path: Path) -> None:
    """A missing rule path must fail closed before any validator is launched."""

    marker = tmp_path / "promtool-called"
    write_fake_executable(
        tmp_path / "promtool",
        'touch "${PROMTOOL_CALLED}"\n',
    )

    result = subprocess.run(
        [
            str(CHECKER),
            "dashboards/alerts/sorafs_fetch_rules.yml",
            "dashboards/alerts/missing-sorafs-rules.yml",
        ],
        cwd=REPO_ROOT,
        env=checker_env(tmp_path, PROMTOOL_CALLED=str(marker)),
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 1
    assert "Rules file not found:" in result.stderr
    assert not marker.exists()


def test_docker_fallback_invokes_the_promtool_entrypoint(tmp_path: Path) -> None:
    """The container fallback must run promtool from the read-only workspace."""

    args_out = tmp_path / "docker-args"
    write_fake_executable(
        tmp_path / "docker",
        'printf "%s\\n" "$@" > "${DOCKER_ARGS_OUT}"\n',
    )
    rule_file = "dashboards/alerts/sorafs_gateway_rules.yml"

    subprocess.run(
        [str(CHECKER), rule_file],
        cwd=REPO_ROOT,
        env=checker_env(tmp_path, DOCKER_ARGS_OUT=str(args_out)),
        check=True,
        text=True,
        capture_output=True,
    )

    assert args_out.read_text(encoding="utf-8").splitlines() == [
        "run",
        "--rm",
        "--entrypoint",
        "/bin/promtool",
        "-v",
        f"{REPO_ROOT}:/workspace:ro",
        "--workdir",
        "/workspace",
        "prom/prometheus",
        "check",
        "rules",
        rule_file,
    ]
