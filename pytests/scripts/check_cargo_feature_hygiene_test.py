"""Tests for the Cargo feature-ownership guard."""

from __future__ import annotations

import importlib.util
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = ROOT / "scripts" / "check_cargo_feature_hygiene.py"
SPEC = importlib.util.spec_from_file_location("check_cargo_feature_hygiene", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
FEATURE_HYGIENE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FEATURE_HYGIENE)


def test_repository_feature_hygiene() -> None:
    assert FEATURE_HYGIENE.check_repository(ROOT) == []


def _write_fixture(root: Path, *, root_features: bool = False, member_defaults: bool = False) -> None:
    dependency_rows = []
    member_rows = []
    for name in sorted(FEATURE_HYGIENE.FOUNDATIONAL_DEPENDENCIES):
        injected = ', features = ["broad"]' if root_features and name == "norito" else ""
        dependency_rows.append(
            f'{name} = {{ path = "deps/{name}", default-features = false{injected} }}'
        )
        default_setting = "" if member_defaults and name == "norito" else ", default-features = false"
        member_rows.append(f"{name} = {{ workspace = true{default_setting} }}")

    (root / "crates" / "consumer").mkdir(parents=True)
    (root / "Cargo.toml").write_text(
        "\n".join(
            [
                "[workspace]",
                'default-members = ["crates/consumer"]',
                "",
                "[workspace.dependencies]",
                *dependency_rows,
                "",
            ]
        ),
        encoding="utf-8",
    )
    (root / "crates" / "consumer" / "Cargo.toml").write_text(
        "\n".join(
            [
                "[package]",
                'name = "consumer"',
                'version = "0.1.0"',
                "",
                "[dependencies]",
                *member_rows,
                "",
            ]
        ),
        encoding="utf-8",
    )


def test_accepts_explicit_default_member_feature_ownership(tmp_path: Path) -> None:
    _write_fixture(tmp_path)

    assert FEATURE_HYGIENE.check_repository(tmp_path) == []


def test_rejects_workspace_feature_injection(tmp_path: Path) -> None:
    _write_fixture(tmp_path, root_features=True)

    errors = FEATURE_HYGIENE.check_repository(tmp_path)

    assert any("workspace dependency `norito` must not inject features" in error for error in errors)


def test_rejects_implicit_default_features_in_default_member(tmp_path: Path) -> None:
    _write_fixture(tmp_path, member_defaults=True)

    errors = FEATURE_HYGIENE.check_repository(tmp_path)

    assert any("[dependencies] `norito` must set `default-features = false`" in error for error in errors)
