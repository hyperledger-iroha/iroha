"""Static deployment contracts for mandatory Taira offline admission."""

from __future__ import annotations

import tomllib
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
TAIRA_DIR = REPO_ROOT / "configs" / "soranexus" / "taira"


def test_compose_mounts_exact_mandatory_offline_inputs_without_creation() -> None:
    config = tomllib.loads((TAIRA_DIR / "config.toml").read_text(encoding="utf-8"))
    offline = config["settlement"]["offline"]
    compose = (TAIRA_DIR / "docker-compose.validator.yml").read_text(
        encoding="utf-8"
    )

    assert (
        "${TAIRA_KAGEMUSHA_RELEASE_POLICY_PATH:?"
        "set TAIRA_KAGEMUSHA_RELEASE_POLICY_PATH"
    ) in compose
    assert (
        "${TAIRA_KAGEMUSHA_ARTIFACT_DIR:?set TAIRA_KAGEMUSHA_ARTIFACT_DIR"
    ) in compose
    assert "${TAIRA_CONFIG_PATH:?set TAIRA_CONFIG_PATH" in compose
    assert "${TAIRA_STORAGE_PATH:?set TAIRA_STORAGE_PATH" in compose
    assert f"target: {offline['kagemusha_release_policy_path']}" in compose
    assert f"target: {offline['kagemusha_artifact_dir']}" in compose
    assert compose.count("create_host_path: false") == 4
    assert compose.count("read_only: true") >= 3


def test_compose_healthcheck_uses_readiness_not_process_liveness_or_status() -> None:
    compose = (TAIRA_DIR / "docker-compose.validator.yml").read_text(
        encoding="utf-8"
    )
    healthcheck = compose.split("    healthcheck:\n", 1)[1]

    assert "http://127.0.0.1:8080/readyz" in healthcheck
    assert "/livez" not in next(
        line for line in healthcheck.splitlines() if 'test: ["CMD"' in line
    )
    assert "/status" not in next(
        line for line in healthcheck.splitlines() if 'test: ["CMD"' in line
    )


def test_compose_wrapper_waits_and_fails_closed() -> None:
    wrapper = (TAIRA_DIR / "taira-validator-compose.sh").read_text(
        encoding="utf-8"
    )

    assert "--wait --wait-timeout" in wrapper
    assert "config --format json" in wrapper
    assert "only the reviewed mandatory-offline Compose file is allowed" in wrapper
    assert "http://127.0.0.1:8080/readyz" in wrapper
    assert "rm --stop --force taira-validator" in wrapper
    assert "mandatory /readyz admission" in wrapper


def test_runbook_has_no_direct_compose_or_nginx_cutover_bypass() -> None:
    runbook = (TAIRA_DIR / "README.md").read_text(encoding="utf-8")

    assert "docker compose --env-file" not in runbook
    assert "sudo cp dist/taira-edge" not in runbook
    assert "sudo nginx -t && sudo systemctl reload nginx" not in runbook
    assert "nginx -t && nginx -s reload" not in runbook
    assert "direct `docker compose up` is not an authorized" in runbook
    assert "all-validator exact-identity admission" in runbook
