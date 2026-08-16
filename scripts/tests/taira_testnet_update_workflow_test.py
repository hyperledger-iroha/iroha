"""Static contract for the fast, routine Taira testnet workflow."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/update_taira_testnet.yml"
FULL_RELEASE_WORKFLOW = ROOT / ".github/workflows/publish_taira_validator.yml"


def _workflow() -> dict[str, object]:
    command = (
        "document = YAML.safe_load(STDIN.read, aliases: false); "
        'puts JSON.generate(document.fetch("jobs"))'
    )
    result = subprocess.run(
        ["ruby", "-ryaml", "-rjson", "-e", command],
        input=WORKFLOW.read_text(encoding="utf-8"),
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def test_routine_update_is_one_build_cutover_job_plus_deadline_watcher() -> None:
    jobs = _workflow()
    assert list(jobs) == ["budget", "update"]
    update = jobs["update"]
    assert update["name"] == "Taira testnet update"
    assert update["runs-on"] == ["self-hosted", "macOS", "ARM64", "taira-deploy"]
    assert "environment" not in update
    assert update["timeout-minutes"] == 30
    assert "needs" not in update
    assert update["concurrency"] == {
        "group": "taira-validator-rollout",
        "cancel-in-progress": False,
    }

    text = json.dumps(update, sort_keys=True)
    assert text.count("cargo build") == 1
    assert "-p irohad --bin iroha3d" in text
    assert "embedded-soracloud-runtime,zk-stark" in text
    assert "/usr/local/libexec/iroha-taira-testnet-update-v1" not in text
    source = WORKFLOW.read_text(encoding="utf-8")
    assert 'sudo -n "$TAIRA_TESTNET_UPDATER"' in source
    assert '--deadline-seconds "$helper_budget"' in source
    assert "27 * 60" in source
    assert "helper_budget < 420" in source
    assert "--apply" in text
    for forbidden in (
        "privacy",
        "boi",
        "authority",
        "linux",
        "artifact",
        "deploy-reset",
        "prepare-reset",
        "check_taira_release_prerequisites",
        "upload-artifact",
        "download-artifact",
    ):
        assert forbidden not in text.lower()


def test_queue_time_and_build_are_inside_the_thirty_minute_budget() -> None:
    jobs = _workflow()
    budget = jobs["budget"]
    assert budget["runs-on"] == "ubuntu-latest"
    assert budget["timeout-minutes"] == 31
    assert budget["permissions"] == {"actions": "write", "contents": "none"}
    text = json.dumps(budget, sort_keys=True)
    assert "30 * 60" in text
    assert "2 * 60" in text
    assert "/cancel" in text
    assert "taira-deploy runner did not start" in text


def test_full_release_is_visibly_exceptional_and_serialized_with_updates() -> None:
    fast = WORKFLOW.read_text(encoding="utf-8")
    full = FULL_RELEASE_WORKFLOW.read_text(encoding="utf-8")
    assert full.startswith("name: Taira Full Release/Reset (exceptional)\n")
    concurrency = "group: taira-validator-rollout"
    assert concurrency in fast
    assert concurrency in full
