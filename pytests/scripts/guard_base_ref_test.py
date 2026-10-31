"""The contributor guards share one comparison base and refuse to run without ripgrep."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[2]
BASE_REF_LIB = ROOT / "ci" / "lib" / "base_ref.sh"
GUARDS = (
    "check_agents_guardrails",
    "check_dependency_discipline",
    "check_env_config_surface",
    "check_missing_docs_guard",
    "check_serde_guard",
    "check_status_sync",
    "check_todo_guard",
)


def git(root: Path, *arguments: str) -> None:
    subprocess.run(("git", *arguments), cwd=root, check=True, capture_output=True)


def resolve(root: Path, **environment: str) -> str:
    done = subprocess.run(
        ["bash", "-c", f'source "{BASE_REF_LIB}"; resolve_base'],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
        env={**{k: v for k, v in os.environ.items() if k != "AGENTS_BASE_REF"}, **environment},
    )
    return done.stdout.strip()


@pytest.fixture
def repository(tmp_path: Path) -> Path:
    git(tmp_path, "init", "-q")
    git(tmp_path, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "a")
    return tmp_path


def test_configured_branch_is_preferred_over_stale_default_branches(repository: Path) -> None:
    git(repository, "update-ref", "refs/remotes/origin/main", "HEAD")
    assert resolve(repository) == "origin/main"
    git(repository, "update-ref", "refs/remotes/origin/optimizations", "HEAD")
    assert resolve(repository) == "origin/optimizations"


def test_override_wins_and_a_missing_override_falls_back(repository: Path) -> None:
    git(repository, "update-ref", "refs/remotes/origin/optimizations", "HEAD")
    git(repository, "update-ref", "refs/heads/feature", "HEAD")
    assert resolve(repository, AGENTS_BASE_REF="feature") == "feature"
    assert resolve(repository, AGENTS_BASE_REF="no-such-ref") == "origin/optimizations"


def test_clone_without_any_known_branch_uses_the_previous_commit(repository: Path) -> None:
    git(repository, "branch", "-m", "topic")
    assert resolve(repository) == "HEAD^"


@pytest.mark.parametrize("guard", GUARDS)
def test_every_guard_uses_the_shared_base_ref(guard: str) -> None:
    source = (ROOT / "ci" / f"{guard}.sh").read_text(encoding="utf-8")
    assert 'source "${ROOT}/ci/lib/base_ref.sh"' in source
    assert "resolve_base() {" not in source


@pytest.mark.parametrize(
    "script",
    (
        "scripts/deny_serde_json.sh",
        "scripts/deny_handrolled_aos.sh",
        "scripts/check_contract_address_chain_identity.sh",
        "ci/check_soranet_privacy_guard.sh",
    ),
)
def test_guards_report_missing_ripgrep_instead_of_passing_or_blaming_the_code(
    script: str, tmp_path: Path
) -> None:
    tools = tmp_path / "tools"
    tools.mkdir()
    for tool in ("bash", "dirname", "git"):
        located = shutil.which(tool)
        assert located is not None
        (tools / tool).symlink_to(located)
    done = subprocess.run(
        [str(tools / "bash"), str(ROOT / script)],
        cwd=ROOT,
        capture_output=True,
        text=True,
        env={"PATH": str(tools), "HOME": str(tmp_path)},
    )
    assert done.returncode == 2, done.stdout + done.stderr
    assert "ripgrep (rg) is required" in done.stderr
    assert "brew install ripgrep" in done.stderr
