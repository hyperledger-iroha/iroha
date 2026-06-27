"""Tests for shared SoraFS path identity helpers."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_path_identity import resolve_path_identity  # noqa: E402


def test_resolve_path_identity_returns_canonical_path(tmp_path: Path) -> None:
    target = tmp_path / "file.txt"
    target.write_text("ok", encoding="utf-8")
    child = tmp_path / "nested" / ".." / "file.txt"

    errors: list[str] = []

    assert resolve_path_identity(child, errors) == target.resolve()
    assert errors == []


def test_resolve_path_identity_rejects_non_path_without_traceback() -> None:
    errors: list[str] = []

    assert resolve_path_identity("reviewed.args", errors, label="@ARGFILE") is None
    assert errors == ["@ARGFILE `reviewed.args` must be a path"]


def test_resolve_path_identity_rejects_malformed_error_container(
    tmp_path: Path,
) -> None:
    target = tmp_path / "file.txt"
    target.write_text("ok", encoding="utf-8")

    for errors in ("", (), {"error": "old"}, ["old", 7]):
        try:
            resolve_path_identity(target, errors)
        except ValueError as error:
            assert "path identity errors must be a list of strings" in str(error)
        else:
            raise AssertionError(f"accepted malformed errors {errors!r}")


def test_resolve_path_identity_rejects_malformed_labels_before_resolution(
    tmp_path: Path,
    monkeypatch,
) -> None:
    target = tmp_path / "file.txt"
    target.write_text("ok", encoding="utf-8")

    def resolve(self: Path, *args, **kwargs):
        raise AssertionError("resolve should not run for malformed labels")

    monkeypatch.setattr(Path, "resolve", resolve)

    for label in ("", " path", "path ", "path\nname", 7):
        errors: list[str] = []
        try:
            resolve_path_identity(target, errors, label=label)
        except ValueError as error:
            assert "path identity label must be a non-empty canonical string" in str(
                error
            )
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed label {label!r}")


def test_resolve_path_identity_rejects_malformed_failure_templates_before_resolution(
    tmp_path: Path,
    monkeypatch,
) -> None:
    target = tmp_path / "file.txt"
    target.write_text("ok", encoding="utf-8")

    def resolve(self: Path, *args, **kwargs):
        raise AssertionError("resolve should not run for malformed templates")

    monkeypatch.setattr(Path, "resolve", resolve)

    for template, expected in [
        ("", "failure template must be a non-empty string"),
        (7, "failure template must be a non-empty string"),
        ("failed {path}", "failure template must include {path} and {error}"),
        ("failed {error}", "failure template must include {path} and {error}"),
        (
            "failed {path}: {error} via {missing}",
            "failure template fields must be label, path, or error",
        ),
        ("failed {path}: {error", "failure template must be valid format text"),
        (
            "failed {path!r}: {error}",
            "failure template fields must not use format specifiers",
        ),
        (
            "failed {path:s}: {error}",
            "failure template fields must not use format specifiers",
        ),
    ]:
        errors: list[str] = []
        try:
            resolve_path_identity(target, errors, failure_template=template)
        except ValueError as error:
            assert expected in str(error)
            assert errors == []
        else:
            raise AssertionError(f"accepted malformed template {template!r}")


def test_resolve_path_identity_records_custom_failure(
    tmp_path: Path,
    monkeypatch,
) -> None:
    argfile = tmp_path / "reviewed.args"
    original_resolve = Path.resolve

    def resolve(self: Path, *args, **kwargs):
        if self == argfile:
            raise RuntimeError("identity denied")
        return original_resolve(self, *args, **kwargs)

    monkeypatch.setattr(Path, "resolve", resolve)

    errors: list[str] = []

    assert (
        resolve_path_identity(
            argfile,
            errors,
            label="@ARGFILE",
            failure_template="failed to resolve @ARGFILE `{path}`: {error}",
        )
        is None
    )
    assert errors == [f"failed to resolve @ARGFILE `{argfile}`: identity denied"]
