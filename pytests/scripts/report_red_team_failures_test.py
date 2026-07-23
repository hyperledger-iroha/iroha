"""Regression tests for the red-team failure-reporting helper."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import sys
import urllib.error
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "report_red_team_failures.py"
SPEC = importlib.util.spec_from_file_location("report_red_team_failures", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
REPORTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORTER)


def test_copy_logs_preserves_same_named_sources(tmp_path: Path) -> None:
    first = tmp_path / "first" / "run.log"
    second = tmp_path / "second" / "run.log"
    first.parent.mkdir()
    second.parent.mkdir()
    first.write_text("first\n", encoding="utf-8")
    second.write_text("second\n", encoding="utf-8")

    copied = REPORTER._copy_logs([first, second], tmp_path / "artifacts")

    assert len(copied) == 2
    assert copied[0].name != copied[1].name
    assert {path.read_text(encoding="utf-8") for path in copied} == {"first\n", "second\n"}


def test_copy_logs_avoids_hash_prefix_collision_with_literal_basename(
    tmp_path: Path,
) -> None:
    first = tmp_path / "first" / "run.log"
    second = tmp_path / "second" / "run.log"
    first.parent.mkdir()
    second.parent.mkdir()
    first.write_text("first\n", encoding="utf-8")
    second.write_text("second\n", encoding="utf-8")

    first_hash = hashlib.sha256(str(first.resolve()).encode("utf-8")).hexdigest()[:12]
    literal = tmp_path / "third" / f"{first_hash}-run.log"
    literal.parent.mkdir()
    literal.write_text("literal\n", encoding="utf-8")

    copied = REPORTER._copy_logs([first, literal, second], tmp_path / "artifacts")

    assert len(copied) == 3
    assert len({path.name for path in copied}) == 3
    assert literal.name in {path.name for path in copied}
    assert {path.read_text(encoding="utf-8") for path in copied} == {
        "first\n",
        "second\n",
        "literal\n",
    }


def test_copy_logs_does_not_overwrite_existing_archive(tmp_path: Path) -> None:
    source = tmp_path / "source" / "run.log"
    source.parent.mkdir()
    source.write_text("new\n", encoding="utf-8")
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    existing = artifact_dir / "run.log"
    existing.write_text("existing\n", encoding="utf-8")

    copied = REPORTER._copy_logs([source], artifact_dir)

    assert existing.read_text(encoding="utf-8") == "existing\n"
    assert len(copied) == 1
    assert copied[0].name != existing.name
    assert copied[0].read_text(encoding="utf-8") == "new\n"


@pytest.mark.parametrize(
    "failure",
    [
        urllib.error.URLError("offline"),
        urllib.error.HTTPError(
            "https://api.github.test/issues",
            503,
            "unavailable",
            None,
            io.BytesIO(b"try later"),
        ),
    ],
)
def test_main_fails_when_issue_creation_fails(
    monkeypatch: pytest.MonkeyPatch,
    failure: Exception,
) -> None:
    def fail_request(_request: object) -> object:
        raise failure

    monkeypatch.setattr(REPORTER.urllib.request, "urlopen", fail_request)
    monkeypatch.setenv("REPORTER_TEST_TOKEN", "token")
    monkeypatch.setattr(
        sys,
        "argv",
        [
            str(SCRIPT),
            "--surface",
            "nightly suite",
            "--mitigation",
            "inspect logs",
            "--repo",
            "example/repository",
            "--token-env",
            "REPORTER_TEST_TOKEN",
        ],
    )

    assert REPORTER.main() == 1


def test_create_issue_rejects_non_success_status(monkeypatch: pytest.MonkeyPatch) -> None:
    class Response:
        status = 500

        def __enter__(self) -> "Response":
            return self

        def __exit__(self, *_args: object) -> None:
            return None

    monkeypatch.setattr(REPORTER.urllib.request, "urlopen", lambda _request: Response())

    assert not REPORTER._create_issue("token", "example/repository", "title", "body", [])


@pytest.mark.parametrize("missing", ["token", "repository"])
def test_main_fails_when_issue_configuration_is_missing(
    monkeypatch: pytest.MonkeyPatch,
    missing: str,
) -> None:
    token_env = "REPORTER_TEST_TOKEN"
    if missing != "token":
        monkeypatch.setenv(token_env, "token")
    else:
        monkeypatch.delenv(token_env, raising=False)

    arguments = [
        str(SCRIPT),
        "--surface",
        "nightly suite",
        "--mitigation",
        "inspect logs",
        "--token-env",
        token_env,
    ]
    if missing != "repository":
        arguments.extend(["--repo", "example/repository"])
    else:
        monkeypatch.delenv("GITHUB_REPOSITORY", raising=False)
    monkeypatch.setattr(sys, "argv", arguments)

    assert REPORTER.main() == 1
