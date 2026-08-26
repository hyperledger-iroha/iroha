"""Regression tests for the Taikai Grafana dashboard exporter."""

from __future__ import annotations

import importlib.util
import io
import json
import stat
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "grafana" / "export_taikai_viewer.py"
SPEC = importlib.util.spec_from_file_location("export_taikai_viewer", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
EXPORTER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = EXPORTER
SPEC.loader.exec_module(EXPORTER)


class _Response:
    def __init__(self, payload: bytes, headers: dict[str, str] | None = None) -> None:
        self._payload = io.BytesIO(payload)
        self.headers = headers or {}

    def read(self, size: int = -1) -> bytes:
        return self._payload.read(size)

    def __enter__(self) -> "_Response":
        return self

    def __exit__(self, *_args: object) -> None:
        return None


class _Opener:
    def __init__(self, response: _Response) -> None:
        self.response = response
        self.request = None

    def open(self, request, timeout: int):  # noqa: ANN001, ANN201
        assert timeout == 30
        self.request = request
        return self.response


def _install_response(
    monkeypatch: pytest.MonkeyPatch,
    payload: object,
    headers: dict[str, str] | None = None,
) -> _Opener:
    response = _Response(json.dumps(payload).encode("utf-8"), headers)
    opener = _Opener(response)

    def build_opener(handler):  # noqa: ANN001, ANN202
        assert isinstance(handler, EXPORTER._NoRedirectHandler)
        return opener

    monkeypatch.setattr(EXPORTER.request, "build_opener", build_opener)
    return opener


def test_fetch_dashboard_bounds_uid_and_strips_volatile_fields(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    opener = _install_response(
        monkeypatch,
        {
            "dashboard": {
                "uid": "team/main",
                "title": "Taikai",
                "id": 7,
                "iteration": 8,
                "version": 9,
            }
        },
    )

    dashboard = EXPORTER.fetch_dashboard(
        "https://grafana.example.test", "secret", "team/main"
    )

    assert dashboard == {"uid": "team/main", "title": "Taikai"}
    assert opener.request.full_url.endswith("/api/dashboards/uid/team%2Fmain")
    assert opener.request.get_header("Authorization") == "Bearer secret"


def test_fetch_dashboard_rejects_mismatched_response_uid(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _install_response(monkeypatch, {"dashboard": {"uid": "other"}})

    with pytest.raises(SystemExit, match="does not match requested"):
        EXPORTER.fetch_dashboard("https://grafana.example.test", "secret", "expected")


def test_fetch_dashboard_rejects_oversized_declared_response(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _install_response(
        monkeypatch,
        {"dashboard": {"uid": "taikai"}},
        {"Content-Length": str(EXPORTER.MAX_DASHBOARD_BYTES + 1)},
    )

    with pytest.raises(SystemExit, match="exceeds"):
        EXPORTER.fetch_dashboard("https://grafana.example.test", "secret", "taikai")


def test_write_dashboard_rejects_symlink_and_preserves_target(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    target.write_text("preserve\n", encoding="utf-8")
    destination = tmp_path / "dashboard.json"
    try:
        destination.symlink_to(target)
    except OSError as error:
        pytest.skip(f"symlink creation unavailable: {error}")

    with pytest.raises(SystemExit, match="regular file or absent"):
        EXPORTER.write_dashboard({"uid": "taikai"}, destination)
    assert target.read_text(encoding="utf-8") == "preserve\n"


def test_write_dashboard_publishes_atomically(tmp_path: Path) -> None:
    destination = tmp_path / "dashboard.json"
    EXPORTER.write_dashboard({"uid": "taikai", "title": "Viewer"}, destination)

    assert json.loads(destination.read_text(encoding="utf-8")) == {
        "uid": "taikai",
        "title": "Viewer",
    }
    assert stat.S_IMODE(destination.stat().st_mode) == 0o644
    assert not list(tmp_path.glob(".dashboard.json.tmp-*"))


def test_write_dashboard_preserves_existing_mode(tmp_path: Path) -> None:
    destination = tmp_path / "dashboard.json"
    destination.write_text("{}\n", encoding="utf-8")
    destination.chmod(0o640)

    EXPORTER.write_dashboard({"uid": "taikai"}, destination)

    assert stat.S_IMODE(destination.stat().st_mode) == 0o640
