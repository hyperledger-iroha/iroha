"""Syntax guards for Python programs copied into Inrou guest bundles."""

from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIR = (
    REPO_ROOT / "crates/irohad/src/soracloud_runtime/tests/fixtures"
)
FIXTURES = (
    FIXTURE_DIR / "inrou_health_server.py",
)


@pytest.mark.parametrize("fixture", FIXTURES, ids=lambda path: path.stem)
def test_inrou_guest_python_fixture_compiles(fixture: Path) -> None:
    source_bytes = fixture.read_bytes()
    assert source_bytes.endswith(b"\n"), "shell heredoc delimiter requires a trailing newline"
    source = source_bytes.decode("utf-8")

    compile(source, str(fixture), "exec")
